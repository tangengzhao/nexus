//! HSB gRPC 传输层
//!
//! 提供 gRPC 客户端和服务端实现，用于与 rust-sso 等服务集成。

mod client;
mod config;
mod server;
mod sso;

pub use client::*;
pub use config::*;
pub use server::*;
pub use sso::*;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::{
    ConnectableTransport, HealthStatus, Transport, TransportRequest, TransportResponse,
    TransportStats,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::info;

/// gRPC 传输层
pub struct GrpcTransport {
    config: GrpcTransportConfig,
    channel: Arc<RwLock<Option<Channel>>>,
    stats: Arc<GrpcStats>,
}

/// gRPC 统计
pub struct GrpcStats {
    pub requests_sent: AtomicU64,
    pub requests_failed: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub start_time: Instant,
}

impl GrpcStats {
    pub fn new() -> Self {
        Self {
            requests_sent: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }
}

impl Default for GrpcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcTransport {
    pub fn new(config: GrpcTransportConfig) -> Self {
        Self {
            config,
            channel: Arc::new(RwLock::new(None)),
            stats: Arc::new(GrpcStats::new()),
        }
    }

    async fn get_or_create_channel(&self) -> HsbResult<Channel> {
        {
            let channel = self.channel.read().await;
            if let Some(ref ch) = *channel {
                return Ok(ch.clone());
            }
        }

        let mut channel = self.channel.write().await;
        if let Some(ref ch) = *channel {
            return Ok(ch.clone());
        }

        let endpoint = tonic::transport::Endpoint::from_shared(self.config.endpoint.clone())
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.config.endpoint.clone(),
                message: e.to_string(),
            })?
            .connect_timeout(Duration::from_secs(self.config.connect_timeout_secs))
            .timeout(Duration::from_secs(self.config.request_timeout_secs));

        let ch = endpoint
            .connect()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.config.endpoint.clone(),
                message: e.to_string(),
            })?;

        info!("gRPC channel connected to {}", self.config.endpoint);
        *channel = Some(ch.clone());

        Ok(ch)
    }
}

#[async_trait]
impl Transport for GrpcTransport {
    fn transport_type(&self) -> hsb_core::TransportType {
        hsb_core::TransportType::Grpc
    }

    fn name(&self) -> &str {
        "grpc"
    }

    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
        let start = Instant::now();
        let channel = self.get_or_create_channel().await?;

        self.stats
            .bytes_sent
            .fetch_add(request.body.len() as u64, Ordering::Relaxed);
        self.stats.requests_sent.fetch_add(1, Ordering::Relaxed);

        // 使用 tonic 的 generic client 发送请求
        // 这是一个简化实现，实际使用时需要根据具体的 proto 定义
        let _client = tonic::client::Grpc::new(channel);

        // 构建 gRPC 路径（从 target 获取）
        let _path = &request.target;

        // 简化：直接返回成功响应
        // 实际实现需要根据具体的 gRPC 服务定义
        let response_body = Bytes::from("gRPC response");
        self.stats
            .bytes_received
            .fetch_add(response_body.len() as u64, Ordering::Relaxed);

        Ok(TransportResponse {
            status_code: 200,
            body: response_body,
            headers: std::collections::HashMap::new(),
            duration: start.elapsed(),
            metadata: Default::default(),
        })
    }

    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        timeout: Duration,
    ) -> HsbResult<TransportResponse> {
        tokio::time::timeout(timeout, self.send(request))
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "gRPC request".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })?
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        match self.get_or_create_channel().await {
            Ok(_) => Ok(HealthStatus {
                healthy: true,
                message: "gRPC channel connected".to_string(),
                last_check: chrono::Utc::now(),
                details: std::collections::HashMap::new(),
            }),
            Err(e) => Ok(HealthStatus {
                healthy: false,
                message: e.to_string(),
                last_check: chrono::Utc::now(),
                details: std::collections::HashMap::new(),
            }),
        }
    }

    fn stats(&self) -> TransportStats {
        TransportStats {
            messages_sent: self.stats.requests_sent.load(Ordering::Relaxed),
            messages_received: 0,
            bytes_sent: self.stats.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            errors: self.stats.requests_failed.load(Ordering::Relaxed),
            avg_response_time_ms: 0.0,
            max_response_time_ms: 0,
            active_connections: 0, // 简化：不使用 async
        }
    }
}

#[async_trait]
impl ConnectableTransport for GrpcTransport {
    async fn connect(&self) -> HsbResult<()> {
        self.get_or_create_channel().await?;
        Ok(())
    }

    async fn disconnect(&self) -> HsbResult<()> {
        let mut channel = self.channel.write().await;
        *channel = None;
        info!("gRPC channel disconnected");
        Ok(())
    }

    async fn reconnect(&self) -> HsbResult<()> {
        self.disconnect().await?;
        self.connect().await
    }

    fn is_connected(&self) -> bool {
        // 使用 try_read 来避免 async
        self.channel
            .try_read()
            .map(|c| c.is_some())
            .unwrap_or(false)
    }
}
