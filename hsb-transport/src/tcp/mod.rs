//! HSB TCP/MLLP 传输
//!
//! 提供 TCP 和 MLLP（Minimal Lower Layer Protocol）传输实现。
//! MLLP 是 HL7 v2.x 消息的标准传输协议。

mod client;
mod config;
mod mllp;
mod server;

pub use client::TcpClient;
pub use config::*;
pub use mllp::{MllpCodec, MllpFrame};
pub use server::TcpServer;

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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

/// TCP/MLLP 传输
pub struct TcpMllpTransport {
    config: TcpTransportConfig,
    connection: Arc<RwLock<Option<TcpStream>>>,
    connected: AtomicBool,
    stats: TransportStatsInner,
}

#[derive(Default)]
struct TransportStatsInner {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    errors: AtomicU64,
    total_response_time_ms: AtomicU64,
    request_count: AtomicU64,
    max_response_time_ms: AtomicU64,
}

impl TcpMllpTransport {
    pub fn new(config: TcpTransportConfig) -> Self {
        Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            connected: AtomicBool::new(false),
            stats: TransportStatsInner::default(),
        }
    }

    fn update_stats(
        &self,
        bytes_sent: u64,
        bytes_received: u64,
        duration: Duration,
        is_error: bool,
    ) {
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_sent
            .fetch_add(bytes_sent, Ordering::Relaxed);
        self.stats
            .bytes_received
            .fetch_add(bytes_received, Ordering::Relaxed);

        let duration_ms = duration.as_millis() as u64;
        self.stats
            .total_response_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        self.stats.request_count.fetch_add(1, Ordering::Relaxed);

        let current_max = self.stats.max_response_time_ms.load(Ordering::Relaxed);
        if duration_ms > current_max {
            self.stats
                .max_response_time_ms
                .store(duration_ms, Ordering::Relaxed);
        }

        if is_error {
            self.stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[async_trait]
impl Transport for TcpMllpTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::TcpMllp
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
        let timeout = request
            .timeout
            .unwrap_or(Duration::from_secs(self.config.timeout_secs));
        self.send_with_timeout(request, timeout).await
    }

    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        timeout: Duration,
    ) -> HsbResult<TransportResponse> {
        let start = Instant::now();

        // 获取或创建连接
        let mut conn_guard = self.connection.write().await;

        if conn_guard.is_none() {
            let addr = format!("{}:{}", self.config.host, self.config.port);
            let stream = tokio::time::timeout(
                Duration::from_secs(self.config.connect_timeout_secs),
                TcpStream::connect(&addr),
            )
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "TCP connect".to_string(),
                timeout_ms: self.config.connect_timeout_secs * 1000,
            })?
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to connect: {}", e),
            })?;

            *conn_guard = Some(stream);
            self.connected.store(true, Ordering::Relaxed);
        }

        let stream = conn_guard
            .as_mut()
            .ok_or_else(|| HsbError::TransportError {
                message: "Connection not available".to_string(),
            })?;

        // 发送数据（如果启用 MLLP，则包装）
        let data_to_send = if self.config.use_mllp {
            mllp::wrap_mllp(&request.body)
        } else {
            request.body.to_vec()
        };

        let write_result = tokio::time::timeout(timeout, stream.write_all(&data_to_send)).await;

        match write_result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                *conn_guard = None;
                self.connected.store(false, Ordering::Relaxed);
                self.update_stats(data_to_send.len() as u64, 0, start.elapsed(), true);
                return Err(HsbError::TransportError {
                    message: format!("Write failed: {}", e),
                });
            }
            Err(_) => {
                self.update_stats(data_to_send.len() as u64, 0, start.elapsed(), true);
                return Err(HsbError::TimeoutError {
                    operation: "TCP write".to_string(),
                    timeout_ms: timeout.as_millis() as u64,
                });
            }
        }

        // 读取响应
        let mut buffer = vec![0u8; self.config.buffer_size];
        let read_result = tokio::time::timeout(timeout, stream.read(&mut buffer)).await;

        let duration = start.elapsed();

        match read_result {
            Ok(Ok(n)) => {
                buffer.truncate(n);

                // 如果启用 MLLP，则解包
                let response_data = if self.config.use_mllp {
                    mllp::unwrap_mllp(&buffer).unwrap_or_else(|_| buffer.clone())
                } else {
                    buffer
                };

                self.update_stats(
                    data_to_send.len() as u64,
                    response_data.len() as u64,
                    duration,
                    false,
                );
                self.stats.messages_received.fetch_add(1, Ordering::Relaxed);

                Ok(TransportResponse::success(
                    Bytes::from(response_data),
                    duration,
                ))
            }
            Ok(Err(e)) => {
                *conn_guard = None;
                self.connected.store(false, Ordering::Relaxed);
                self.update_stats(data_to_send.len() as u64, 0, duration, true);
                Err(HsbError::TransportError {
                    message: format!("Read failed: {}", e),
                })
            }
            Err(_) => {
                self.update_stats(data_to_send.len() as u64, 0, duration, true);
                Err(HsbError::TimeoutError {
                    operation: "TCP read".to_string(),
                    timeout_ms: duration.as_millis() as u64,
                })
            }
        }
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        if self.connected.load(Ordering::Relaxed) {
            Ok(HealthStatus::healthy())
        } else {
            Ok(HealthStatus::unhealthy("Not connected"))
        }
    }

    fn stats(&self) -> TransportStats {
        let request_count = self.stats.request_count.load(Ordering::Relaxed);
        let total_time = self.stats.total_response_time_ms.load(Ordering::Relaxed);
        let avg = if request_count > 0 {
            total_time as f64 / request_count as f64
        } else {
            0.0
        };

        TransportStats {
            messages_sent: self.stats.messages_sent.load(Ordering::Relaxed),
            messages_received: self.stats.messages_received.load(Ordering::Relaxed),
            bytes_sent: self.stats.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.stats.bytes_received.load(Ordering::Relaxed),
            errors: self.stats.errors.load(Ordering::Relaxed),
            avg_response_time_ms: avg,
            max_response_time_ms: self.stats.max_response_time_ms.load(Ordering::Relaxed),
            active_connections: if self.connected.load(Ordering::Relaxed) {
                1
            } else {
                0
            },
        }
    }
}

#[async_trait]
impl ConnectableTransport for TcpMllpTransport {
    async fn connect(&self) -> HsbResult<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let stream = tokio::time::timeout(
            Duration::from_secs(self.config.connect_timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| HsbError::TimeoutError {
            operation: "TCP connect".to_string(),
            timeout_ms: self.config.connect_timeout_secs * 1000,
        })?
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to connect: {}", e),
        })?;

        let mut conn = self.connection.write().await;
        *conn = Some(stream);
        self.connected.store(true, Ordering::Relaxed);

        Ok(())
    }

    async fn disconnect(&self) -> HsbResult<()> {
        let mut conn = self.connection.write().await;
        *conn = None;
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    async fn reconnect(&self) -> HsbResult<()> {
        self.disconnect().await?;
        self.connect().await
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_creation() {
        let config = TcpTransportConfig::default();
        let transport = TcpMllpTransport::new(config);
        assert!(!transport.is_connected());
    }
}
