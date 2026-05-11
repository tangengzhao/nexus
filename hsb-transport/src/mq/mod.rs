//! HSB 消息队列传输
//!
//! 支持 RabbitMQ/AMQP 协议的消息传输。

mod config;
mod consumer;
mod producer;

pub use config::*;
pub use consumer::MqConsumer;
pub use producer::MqProducer;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::{
    ConnectableTransport, HealthStatus, Transport, TransportRequest, TransportResponse,
    TransportStats, TransportType,
};
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties, options::*, types::FieldTable,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

/// 消息队列传输
pub struct MqTransport {
    config: MqTransportConfig,
    connection: Arc<RwLock<Option<Connection>>>,
    channel: Arc<RwLock<Option<Channel>>>,
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
}

impl MqTransport {
    pub fn new(config: MqTransportConfig) -> Self {
        Self {
            config,
            connection: Arc::new(RwLock::new(None)),
            channel: Arc::new(RwLock::new(None)),
            connected: AtomicBool::new(false),
            stats: TransportStatsInner::default(),
        }
    }
}

#[async_trait]
impl Transport for MqTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::Mq
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
        let start = Instant::now();

        let channel_guard = self.channel.read().await;
        let channel = channel_guard
            .as_ref()
            .ok_or_else(|| HsbError::TransportError {
                message: "Not connected to message queue".to_string(),
            })?;

        // 从 target 解析 exchange 和 routing key
        let (exchange, routing_key) = parse_target(&request.target);

        let properties = BasicProperties::default()
            .with_content_type("application/octet-stream".into())
            .with_delivery_mode(2); // persistent

        channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                &request.body,
                properties,
            )
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to publish message: {}", e),
            })?
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Publish confirmation failed: {}", e),
            })?;

        let duration = start.elapsed();
        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_sent
            .fetch_add(request.body.len() as u64, Ordering::Relaxed);

        // MQ 是异步的，不等待响应
        Ok(TransportResponse::success(Bytes::new(), duration))
    }

    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        _timeout: Duration,
    ) -> HsbResult<TransportResponse> {
        // MQ 发送通常不需要超时控制
        self.send(request).await
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        if !self.connected.load(Ordering::Relaxed) {
            return Ok(HealthStatus::unhealthy("Not connected"));
        }

        let conn_guard = self.connection.read().await;
        if let Some(ref conn) = *conn_guard {
            if conn.status().connected() {
                Ok(HealthStatus::healthy())
            } else {
                Ok(HealthStatus::unhealthy("Connection lost"))
            }
        } else {
            Ok(HealthStatus::unhealthy("No connection"))
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
impl ConnectableTransport for MqTransport {
    async fn connect(&self) -> HsbResult<()> {
        let uri = format!(
            "amqp://{}:{}@{}:{}/{}",
            self.config.username,
            self.config.password,
            self.config.host,
            self.config.port,
            self.config.vhost
        );

        let connection = Connection::connect(&uri, ConnectionProperties::default())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to connect to RabbitMQ: {}", e),
            })?;

        let channel = connection
            .create_channel()
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to create channel: {}", e),
            })?;

        // 设置 QoS
        channel
            .basic_qos(self.config.prefetch_count, BasicQosOptions::default())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to set QoS: {}", e),
            })?;

        {
            let mut conn_guard = self.connection.write().await;
            *conn_guard = Some(connection);
        }

        {
            let mut chan_guard = self.channel.write().await;
            *chan_guard = Some(channel);
        }

        self.connected.store(true, Ordering::Relaxed);
        info!(
            "Connected to RabbitMQ at {}:{}",
            self.config.host, self.config.port
        );

        Ok(())
    }

    async fn disconnect(&self) -> HsbResult<()> {
        {
            let mut chan_guard = self.channel.write().await;
            if let Some(channel) = chan_guard.take() {
                channel.close(200, "Normal shutdown").await.ok();
            }
        }

        {
            let mut conn_guard = self.connection.write().await;
            if let Some(connection) = conn_guard.take() {
                connection.close(200, "Normal shutdown").await.ok();
            }
        }

        self.connected.store(false, Ordering::Relaxed);
        info!("Disconnected from RabbitMQ");

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

/// 解析目标地址为 exchange 和 routing key
fn parse_target(target: &str) -> (&str, &str) {
    if let Some((exchange, routing_key)) = target.split_once('/') {
        (exchange, routing_key)
    } else {
        ("", target) // 默认使用空 exchange
    }
}

/// 声明队列
pub async fn declare_queue(channel: &Channel, queue_name: &str, durable: bool) -> HsbResult<()> {
    let options = QueueDeclareOptions {
        durable,
        exclusive: false,
        auto_delete: false,
        nowait: false,
        passive: false,
    };

    channel
        .queue_declare(queue_name, options, FieldTable::default())
        .await
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to declare queue: {}", e),
        })?;

    Ok(())
}

/// 声明 Exchange
pub async fn declare_exchange(
    channel: &Channel,
    exchange_name: &str,
    exchange_type: &str,
    durable: bool,
) -> HsbResult<()> {
    use lapin::ExchangeKind;

    let kind = match exchange_type {
        "direct" => ExchangeKind::Direct,
        "fanout" => ExchangeKind::Fanout,
        "topic" => ExchangeKind::Topic,
        "headers" => ExchangeKind::Headers,
        _ => ExchangeKind::Direct,
    };

    let options = ExchangeDeclareOptions {
        durable,
        auto_delete: false,
        internal: false,
        nowait: false,
        passive: false,
    };

    channel
        .exchange_declare(exchange_name, kind, options, FieldTable::default())
        .await
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to declare exchange: {}", e),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_target() {
        assert_eq!(
            parse_target("exchange/routing.key"),
            ("exchange", "routing.key")
        );
        assert_eq!(parse_target("queue_name"), ("", "queue_name"));
    }
}
