//! K8s 服务发现
//!
//! 通过 NATS 实现副本自动发现和心跳检测。

use hsb_common::constants::topics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 副本状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    /// 在线
    Online,
    /// 疑似离线（心跳超时）
    Suspect,
    /// 已离线
    Offline,
}

/// 副本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// 实例 ID（Pod 名称或 ULID）
    pub instance_id: String,
    /// Pod IP
    pub address: String,
    /// 节点角色（leader / follower）
    pub role: String,
    /// 状态
    pub status: PeerStatus,
    /// 最后心跳时间
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    /// 启动时间
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// 元数据（labels, namespace 等）
    pub metadata: HashMap<String, String>,
}

/// 服务发现
pub struct ServiceDiscovery {
    /// 本实例 ID
    instance_id: String,
    /// NATS 客户端
    nats_client: Option<async_nats::Client>,
    /// 已知副本列表
    peers: Arc<RwLock<HashMap<String, PeerInfo>>>,
    /// 心跳间隔（秒）
    heartbeat_interval_secs: u64,
    /// 心跳超时（秒）
    heartbeat_timeout_secs: u64,
    /// NATS 主题前缀（保留用于自定义 subject 场景）
    #[allow(dead_code)]
    subject_prefix: String,
}

impl ServiceDiscovery {
    pub fn new(instance_id: String, subject_prefix: String) -> Self {
        Self {
            instance_id,
            nats_client: None,
            peers: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_interval_secs: 5,
            heartbeat_timeout_secs: 15,
            subject_prefix,
        }
    }

    /// 连接到 NATS 并启动心跳
    pub async fn start(&mut self, nats_url: &str) -> hsb_common::HsbResult<()> {
        let client = async_nats::connect(nats_url).await.map_err(|e| {
            hsb_common::HsbError::TransportError {
                message: format!("Failed to connect to NATS for discovery: {}", e),
            }
        })?;
        self.nats_client = Some(client.clone());

        // 启动心跳发送
        let instance_id = self.instance_id.clone();
        let interval = self.heartbeat_interval_secs;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(interval));
            loop {
                ticker.tick().await;
                let heartbeat = PeerInfo {
                    instance_id: instance_id.clone(),
                    address: String::new(), // TODO: 从环境变量获取 Pod IP
                    role: "follower".to_string(),
                    status: PeerStatus::Online,
                    last_heartbeat: chrono::Utc::now(),
                    started_at: chrono::Utc::now(),
                    metadata: HashMap::new(),
                };
                if let Ok(payload) = serde_json::to_vec(&heartbeat) {
                    let subject = topics::SYSTEM_CLUSTER_HEARTBEAT.to_string();
                    if let Err(e) = client.publish(subject, payload.into()).await {
                        warn!("Failed to publish heartbeat: {}", e);
                    }
                }
            }
        });

        // 启动心跳接收
        let peers = self.peers.clone();
        let timeout = self.heartbeat_timeout_secs;
        let self_id = self.instance_id.clone();
        let sub_client = self.nats_client.as_ref().unwrap().clone();
        let subject = topics::SYSTEM_CLUSTER_HEARTBEAT.to_string();
        tokio::spawn(async move {
            let mut sub = match sub_client.subscribe(subject).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to subscribe to heartbeat: {}", e);
                    return;
                }
            };
            while let Some(msg) = futures_lite::StreamExt::next(&mut sub).await {
                if let Ok(peer) = serde_json::from_slice::<PeerInfo>(&msg.payload) {
                    if peer.instance_id != self_id {
                        peers.write().await.insert(peer.instance_id.clone(), peer);
                    }
                }
            }
        });

        // 启动过期清理
        let peers_cleanup = self.peers.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(timeout));
            loop {
                ticker.tick().await;
                let now = chrono::Utc::now();
                let mut peers = peers_cleanup.write().await;
                peers.retain(|_, p| {
                    let elapsed = (now - p.last_heartbeat).num_seconds() as u64;
                    elapsed < timeout * 2
                });
            }
        });

        info!(instance_id = %self.instance_id, "Cluster discovery started");
        Ok(())
    }

    /// 获取所有在线副本
    pub async fn online_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .read()
            .await
            .values()
            .filter(|p| p.status == PeerStatus::Online)
            .cloned()
            .collect()
    }

    /// 获取副本数量
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }
}
