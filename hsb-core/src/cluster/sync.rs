//! K8s 集群状态同步
//!
//! 通过 NATS 实现副本间的状态同步、路由规则广播、配置变更通知。

use hsb_common::constants::topics;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 集群事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterEvent {
    /// 路由规则变更
    RouteChanged {
        route_id: String,
        action: String, // "add", "update", "delete"
    },
    /// 配置变更
    ConfigChanged { key: String, value: String },
    /// 端点状态变更（熔断器触发等）
    EndpointStatusChanged { endpoint_id: String, healthy: bool },
    /// 领导选举
    LeaderElection { leader_id: String, term: u64 },
    /// 自定义事件
    Custom { event_type: String, payload: String },
}

/// 集群状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    /// 当前领导者
    pub leader_id: Option<String>,
    /// 集群版本号（用于乐观锁）
    pub version: u64,
    /// 上次同步时间
    pub last_sync: chrono::DateTime<chrono::Utc>,
}

impl Default for ClusterState {
    fn default() -> Self {
        Self {
            leader_id: None,
            version: 0,
            last_sync: chrono::Utc::now(),
        }
    }
}

/// 集群同步器
pub struct ClusterSync {
    /// 本实例 ID
    instance_id: String,
    /// NATS 客户端
    nats_client: Option<async_nats::Client>,
    /// 集群状态
    state: Arc<RwLock<ClusterState>>,
    /// NATS 主题前缀（保留用于自定义 subject 场景）
    #[allow(dead_code)]
    subject_prefix: String,
}

impl ClusterSync {
    pub fn new(instance_id: String, subject_prefix: String) -> Self {
        Self {
            instance_id,
            nats_client: None,
            state: Arc::new(RwLock::new(ClusterState::default())),
            subject_prefix,
        }
    }

    /// 连接并启动事件监听
    pub async fn start(&mut self, nats_url: &str) -> hsb_common::HsbResult<()> {
        let client = async_nats::connect(nats_url).await.map_err(|e| {
            hsb_common::HsbError::TransportError {
                message: format!("Failed to connect to NATS for cluster sync: {}", e),
            }
        })?;
        self.nats_client = Some(client.clone());

        // 订阅集群事件
        let state = self.state.clone();
        let subject = topics::SYSTEM_CLUSTER_SYNC.to_string();
        let instance_id = self.instance_id.clone();
        tokio::spawn(async move {
            let mut sub = match client.subscribe(subject).await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to subscribe to cluster events: {}", e);
                    return;
                }
            };
            while let Some(msg) = futures_lite::StreamExt::next(&mut sub).await {
                if let Ok(event) = serde_json::from_slice::<ClusterEvent>(&msg.payload) {
                    info!(instance = %instance_id, ?event, "Received cluster event");
                    match &event {
                        ClusterEvent::LeaderElection { leader_id, .. } => {
                            state.write().await.leader_id = Some(leader_id.clone());
                        }
                        _ => {
                            let mut s = state.write().await;
                            s.version += 1;
                            s.last_sync = chrono::Utc::now();
                        }
                    }
                }
            }
        });

        info!(instance_id = %self.instance_id, "Cluster sync started");
        Ok(())
    }

    /// 广播集群事件
    pub async fn broadcast(&self, event: ClusterEvent) -> hsb_common::HsbResult<()> {
        let client =
            self.nats_client
                .as_ref()
                .ok_or_else(|| hsb_common::HsbError::TransportError {
                    message: "NATS client not connected".to_string(),
                })?;

        let payload =
            serde_json::to_vec(&event).map_err(|e| hsb_common::HsbError::SerializationError {
                message: format!("Failed to serialize cluster event: {}", e),
            })?;

        let subject = topics::SYSTEM_CLUSTER_SYNC.to_string();
        client.publish(subject, payload.into()).await.map_err(|e| {
            hsb_common::HsbError::TransportError {
                message: format!("Failed to broadcast cluster event: {}", e),
            }
        })?;

        Ok(())
    }

    /// 获取当前集群状态
    pub async fn state(&self) -> ClusterState {
        self.state.read().await.clone()
    }

    /// 当前副本是否为领导者
    pub async fn is_leader(&self) -> bool {
        self.state
            .read()
            .await
            .leader_id
            .as_ref()
            .map(|id| id == &self.instance_id)
            .unwrap_or(false)
    }
}
