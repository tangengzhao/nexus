//! HSB HTTP/HTTPS 传输
//!
//! 提供 HTTP/HTTPS 协议的传输实现。

mod client;
mod config;
mod server;

pub use client::HttpClient;
pub use config::*;
pub use server::HttpServer;

use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use hsb_core::{
    HealthStatus, Transport, TransportRequest, TransportResponse, TransportStats, TransportType,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// HTTP 传输
pub struct HttpTransport {
    client: reqwest::Client,
    config: HttpTransportConfig,
    stats: Arc<RwLock<TransportStatsInner>>,
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

impl HttpTransport {
    pub fn new(config: HttpTransportConfig) -> HsbResult<Self> {
        let mut client_builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(config.pool_config.max_connections as usize);

        if let Some(ref proxy) = config.proxy {
            let proxy = reqwest::Proxy::all(proxy).map_err(|e| HsbError::ConfigError {
                message: format!("Invalid proxy: {}", e),
            })?;
            client_builder = client_builder.proxy(proxy);
        }

        if config.disable_certificate_validation {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder.build().map_err(|e| HsbError::ConfigError {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

        Ok(Self {
            client,
            config,
            stats: Arc::new(RwLock::new(TransportStatsInner::default())),
        })
    }

    async fn update_stats(
        &self,
        bytes_sent: u64,
        bytes_received: u64,
        duration: Duration,
        is_error: bool,
    ) {
        let stats = self.stats.read().await;
        stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        stats.bytes_sent.fetch_add(bytes_sent, Ordering::Relaxed);
        stats
            .bytes_received
            .fetch_add(bytes_received, Ordering::Relaxed);

        let duration_ms = duration.as_millis() as u64;
        stats
            .total_response_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
        stats.request_count.fetch_add(1, Ordering::Relaxed);

        // 更新最大响应时间
        let current_max = stats.max_response_time_ms.load(Ordering::Relaxed);
        if duration_ms > current_max {
            stats
                .max_response_time_ms
                .store(duration_ms, Ordering::Relaxed);
        }

        if is_error {
            stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    fn transport_type(&self) -> TransportType {
        if self.config.use_tls {
            TransportType::Https
        } else {
            TransportType::Http
        }
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
        let body_len = request.body.len() as u64;

        let mut req_builder = self
            .client
            .post(&request.target)
            .body(request.body)
            .timeout(timeout);

        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // 添加追踪头
        if let Some(ref trace_id) = request.metadata.trace_id {
            req_builder = req_builder.header("X-Trace-Id", trace_id);
        }

        let result = req_builder.send().await;
        let duration = start.elapsed();

        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let headers = resp
                    .headers()
                    .iter()
                    .filter_map(|(k, v)| {
                        v.to_str()
                            .ok()
                            .map(|v| (k.as_str().to_string(), v.to_string()))
                    })
                    .collect();

                let body = resp.bytes().await.map_err(|e| HsbError::TransportError {
                    message: format!("Failed to read response body: {}", e),
                })?;

                let is_error = status >= 400;
                self.update_stats(body_len, body.len() as u64, duration, is_error)
                    .await;

                Ok(TransportResponse {
                    status_code: status,
                    body,
                    headers,
                    duration,
                    metadata: Default::default(),
                })
            }
            Err(e) => {
                self.update_stats(body_len, 0, duration, true).await;
                Err(HsbError::TransportError {
                    message: format!("HTTP request failed: {}", e),
                })
            }
        }
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        if let Some(ref health_url) = self.config.health_check_url {
            let result = self
                .client
                .get(health_url)
                .timeout(Duration::from_secs(5))
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => Ok(HealthStatus::healthy()),
                Ok(resp) => Ok(HealthStatus::unhealthy(&format!(
                    "Health check returned {}",
                    resp.status()
                ))),
                Err(e) => Ok(HealthStatus::unhealthy(&format!(
                    "Health check failed: {}",
                    e
                ))),
            }
        } else {
            Ok(HealthStatus::healthy().with_detail("note", "No health check URL configured"))
        }
    }

    fn stats(&self) -> TransportStats {
        // 使用 try_read 避免阻塞，如果无法获取锁则返回默认值
        if let Ok(stats) = self.stats.try_read() {
            let request_count = stats.request_count.load(Ordering::Relaxed);
            let total_time = stats.total_response_time_ms.load(Ordering::Relaxed);
            let avg = if request_count > 0 {
                total_time as f64 / request_count as f64
            } else {
                0.0
            };

            TransportStats {
                messages_sent: stats.messages_sent.load(Ordering::Relaxed),
                messages_received: stats.messages_received.load(Ordering::Relaxed),
                bytes_sent: stats.bytes_sent.load(Ordering::Relaxed),
                bytes_received: stats.bytes_received.load(Ordering::Relaxed),
                errors: stats.errors.load(Ordering::Relaxed),
                avg_response_time_ms: avg,
                max_response_time_ms: stats.max_response_time_ms.load(Ordering::Relaxed),
                active_connections: 0,
            }
        } else {
            TransportStats::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_transport_creation() {
        let config = HttpTransportConfig::default();
        let transport = HttpTransport::new(config);
        assert!(transport.is_ok());
    }
}
