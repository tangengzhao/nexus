//! HSB NATS/JetStream 传输层
//!
//! 基于 NATS 和 JetStream 的消息传输，支持：
//! - NATS Core: 轻量级 pub/sub（AtMostOnce 投递语义）
//! - JetStream: 持久化流（AtLeastOnce / ExactlyOnce 投递语义）
//! - Queue Groups: 自动负载均衡
//! - Subject 通配符路由

mod config;
mod jetstream;

pub use config::*;
pub use jetstream::JetStreamManager;

use async_nats::Client;
use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::{
    ConnectableTransport, HealthStatus, Transport, TransportRequest, TransportResponse,
    TransportStats, TransportType,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// NATS 传输层
pub struct NatsTransport {
    config: NatsTransportConfig,
    client: Arc<RwLock<Option<Client>>>,
    jetstream: Arc<RwLock<Option<JetStreamManager>>>,
    connected: AtomicBool,
    stats: NatsStatsInner,
}

#[derive(Default)]
struct NatsStatsInner {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    errors: AtomicU64,
}

impl NatsTransport {
    pub fn new(config: NatsTransportConfig) -> Self {
        Self {
            config,
            client: Arc::new(RwLock::new(None)),
            jetstream: Arc::new(RwLock::new(None)),
            connected: AtomicBool::new(false),
            stats: NatsStatsInner::default(),
        }
    }

    /// 获取 NATS 客户端引用
    pub async fn client(&self) -> HsbResult<Client> {
        let guard = self.client.read().await;
        guard.clone().ok_or_else(|| HsbError::TransportError {
            message: "Not connected to NATS".to_string(),
        })
    }

    /// 获取 JetStream 管理器
    pub async fn jetstream_manager(&self) -> HsbResult<JetStreamManager> {
        let guard = self.jetstream.read().await;
        guard.clone().ok_or_else(|| HsbError::TransportError {
            message: "JetStream not initialized".to_string(),
        })
    }

    /// 发布到 NATS Core subject（fire-and-forget）
    pub async fn publish(&self, subject: &str, payload: Bytes) -> HsbResult<()> {
        let client = self.client().await?;
        client
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("NATS publish failed: {}", e),
            })?;
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 发布到 NATS Core subject 并等待响应（request-reply）
    pub async fn request(
        &self,
        subject: &str,
        payload: Bytes,
        timeout: Duration,
    ) -> HsbResult<Bytes> {
        let client = self.client().await?;
        let response =
            tokio::time::timeout(timeout, client.request(subject.to_string(), payload.into()))
                .await
                .map_err(|_| HsbError::TimeoutError {
                    operation: format!("NATS request to {}", subject),
                    timeout_ms: timeout.as_millis() as u64,
                })?
                .map_err(|e| HsbError::TransportError {
                    message: format!("NATS request failed: {}", e),
                })?;

        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats.messages_received.fetch_add(1, Ordering::Relaxed);
        Ok(Bytes::from(response.payload.to_vec()))
    }

    /// 订阅 NATS Core subject
    pub async fn subscribe(&self, subject: &str) -> HsbResult<async_nats::Subscriber> {
        let client = self.client().await?;
        client
            .subscribe(subject.to_string())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("NATS subscribe failed: {}", e),
            })
    }

    /// 通过 Queue Group 订阅（自动负载均衡）
    pub async fn queue_subscribe(
        &self,
        subject: &str,
        queue_group: &str,
    ) -> HsbResult<async_nats::Subscriber> {
        let client = self.client().await?;
        client
            .queue_subscribe(subject.to_string(), queue_group.to_string())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("NATS queue subscribe failed: {}", e),
            })
    }
}

#[async_trait]
impl Transport for NatsTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::Nats
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
        let start = Instant::now();

        // target 格式: "nats://subject" 或 "jetstream://stream.subject"
        let (mode, subject) = parse_nats_target(&request.target);

        match mode {
            NatsMode::Core => {
                self.publish(&subject, request.body.clone()).await?;
                let duration = start.elapsed();
                self.stats
                    .bytes_sent
                    .fetch_add(request.body.len() as u64, Ordering::Relaxed);
                Ok(TransportResponse::success(Bytes::new(), duration))
            }
            NatsMode::Request => {
                let timeout = request.timeout.unwrap_or(Duration::from_secs(30));
                let response = self
                    .request(&subject, request.body.clone(), timeout)
                    .await?;
                let duration = start.elapsed();
                self.stats
                    .bytes_sent
                    .fetch_add(request.body.len() as u64, Ordering::Relaxed);
                self.stats
                    .bytes_received
                    .fetch_add(response.len() as u64, Ordering::Relaxed);
                Ok(TransportResponse::success(response, duration))
            }
            NatsMode::JetStream => {
                let js = self.jetstream_manager().await?;
                js.publish(&subject, request.body.clone()).await?;
                let duration = start.elapsed();
                self.stats
                    .bytes_sent
                    .fetch_add(request.body.len() as u64, Ordering::Relaxed);
                Ok(TransportResponse::success(Bytes::new(), duration))
            }
        }
    }

    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        timeout: Duration,
    ) -> HsbResult<TransportResponse> {
        tokio::time::timeout(timeout, self.send(request))
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "NATS send".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })?
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        if !self.connected.load(Ordering::Relaxed) {
            return Ok(HealthStatus::unhealthy("Not connected to NATS"));
        }

        let client_guard = self.client.read().await;
        match &*client_guard {
            Some(client) => {
                let state = client.connection_state();
                if matches!(state, async_nats::connection::State::Connected) {
                    Ok(HealthStatus::healthy().with_detail("server", &self.config.urls.join(",")))
                } else {
                    Ok(HealthStatus::unhealthy(&format!("NATS state: {:?}", state)))
                }
            }
            None => Ok(HealthStatus::unhealthy("No NATS client")),
        }
    }

    fn stats(&self) -> TransportStats {
        TransportStats {
            messages_sent: self.stats.messages_sent.load(Ordering::Relaxed),
            messages_received: self.stats.messages_received.load(Ordering::Relaxed),
            bytes_sent: self.stats.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            errors: self.stats.errors.load(Ordering::Relaxed),
            avg_response_time_ms: 0.0,
            max_response_time_ms: 0,
            active_connections: if self.connected.load(Ordering::Relaxed) {
                1
            } else {
                0
            },
        }
    }
}

#[async_trait]
impl ConnectableTransport for NatsTransport {
    async fn connect(&self) -> HsbResult<()> {
        let mut opts = async_nats::ConnectOptions::new();

        if let Some(ref creds) = self.config.credentials_path {
            opts = opts
                .credentials_file(creds)
                .await
                .map_err(|e| HsbError::TransportError {
                    message: format!("Failed to load NATS credentials: {}", e),
                })?;
        }

        if let (Some(user), Some(pass)) = (&self.config.username, &self.config.password) {
            opts = opts.user_and_password(user.clone(), pass.clone());
        }

        if let Some(ref token) = self.config.token {
            opts = opts.token(token.clone());
        }

        opts = opts
            .ping_interval(Duration::from_secs(self.config.ping_interval_secs))
            .request_timeout(Some(Duration::from_secs(self.config.request_timeout_secs)));

        let server_url = self.config.urls.join(",");
        let client = opts
            .connect(&server_url)
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to connect to NATS at {}: {}", server_url, e),
            })?;

        info!("Connected to NATS at {}", server_url);

        // 初始化 JetStream（如果启用）
        if self.config.jetstream.enabled {
            let js_context = async_nats::jetstream::new(client.clone());
            let js_manager = JetStreamManager::new(js_context, self.config.jetstream.clone());
            let mut js_guard = self.jetstream.write().await;
            *js_guard = Some(js_manager);
            info!("JetStream initialized");
        }

        {
            let mut client_guard = self.client.write().await;
            *client_guard = Some(client);
        }

        self.connected.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn disconnect(&self) -> HsbResult<()> {
        // 先清理 JetStream
        {
            let mut js_guard = self.jetstream.write().await;
            *js_guard = None;
        }

        // 断开 NATS 连接
        {
            let mut client_guard = self.client.write().await;
            if let Some(client) = client_guard.take() {
                client.flush().await.ok();
            }
        }

        self.connected.store(false, Ordering::Relaxed);
        info!("Disconnected from NATS");
        Ok(())
    }

    async fn reconnect(&self) -> HsbResult<()> {
        warn!("Reconnecting to NATS...");
        self.disconnect().await?;
        self.connect().await
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

// ============ 辅助类型 ============

enum NatsMode {
    Core,
    Request,
    JetStream,
}

/// 解析 NATS 目标地址
///
/// 支持格式：
/// - `nats://subject.name` → Core pub/sub
/// - `nats-req://subject.name` → Request/Reply
/// - `jetstream://subject.name` → JetStream 持久化发布
/// - `subject.name` → 默认 Core pub/sub
fn parse_nats_target(target: &str) -> (NatsMode, String) {
    if let Some(subject) = target.strip_prefix("jetstream://") {
        (NatsMode::JetStream, subject.to_string())
    } else if let Some(subject) = target.strip_prefix("nats-req://") {
        (NatsMode::Request, subject.to_string())
    } else if let Some(subject) = target.strip_prefix("nats://") {
        (NatsMode::Core, subject.to_string())
    } else {
        (NatsMode::Core, target.to_string())
    }
}
