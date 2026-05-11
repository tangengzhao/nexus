//! HSB Kafka 传输层。
//!
//! 用于对接外部 Kafka 集群，实现消息生产与消费。

mod config;

pub use config::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytes::Bytes;
use futures_lite::StreamExt;
use hsb_common::{HsbError, HsbResult};
use hsb_core::{
    ConnectableTransport, HealthStatus, Transport, TransportRequest, TransportResponse,
    TransportStats, TransportType,
};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::{BorrowedHeaders, Headers, Message, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use tokio::sync::RwLock;
use tracing::{error, info};

/// Kafka 消息。
#[derive(Debug, Clone)]
pub struct KafkaMessage {
    pub topic: String,
    pub key: Option<Vec<u8>>,
    pub payload: Bytes,
    pub partition: i32,
    pub offset: i64,
    pub timestamp: Option<i64>,
    pub headers: Vec<(String, Vec<u8>)>,
}

/// Kafka 传输层。
pub struct KafkaTransport {
    config: KafkaTransportConfig,
    producer: Arc<RwLock<Option<FutureProducer>>>,
    consumer: Arc<RwLock<Option<Arc<StreamConsumer>>>>,
    connected: AtomicBool,
    stats: Arc<KafkaStatsInner>,
}

#[derive(Default)]
struct KafkaStatsInner {
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    errors: AtomicU64,
}

impl KafkaTransport {
    pub fn new(config: KafkaTransportConfig) -> Self {
        Self {
            config,
            producer: Arc::new(RwLock::new(None)),
            consumer: Arc::new(RwLock::new(None)),
            connected: AtomicBool::new(false),
            stats: Arc::new(KafkaStatsInner::default()),
        }
    }

    pub async fn publish(
        &self,
        topic: &str,
        payload: Bytes,
        key: Option<&[u8]>,
        headers: Option<&[(String, String)]>,
    ) -> HsbResult<()> {
        let producer = self.producer().await?;

        let mut record = FutureRecord::to(topic).payload(payload.as_ref());
        if let Some(key) = key {
            record = record.key(key);
        }
        if let Some(headers) = headers {
            let owned_headers = headers.iter().fold(OwnedHeaders::new(), |acc, (k, v)| {
                acc.insert(rdkafka::message::Header {
                    key: k,
                    value: Some(v.as_bytes()),
                })
            });
            record = record.headers(owned_headers);
        }

        producer
            .send(
                record,
                Duration::from_secs(self.config.message_timeout_secs),
            )
            .await
            .map_err(|(e, _)| HsbError::TransportError {
                message: format!("Kafka publish failed: {}", e),
            })?;

        self.stats.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.stats
            .bytes_sent
            .fetch_add(payload.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    pub async fn subscribe(&self, topics: &[String]) -> HsbResult<()> {
        let consumer = self.consumer().await?;
        let topic_refs = topics.iter().map(String::as_str).collect::<Vec<_>>();
        consumer
            .subscribe(&topic_refs)
            .map_err(|e| HsbError::TransportError {
                message: format!("Kafka subscribe failed: {}", e),
            })?;
        Ok(())
    }

    pub async fn start_consumer<F, Fut>(&self, topics: &[String], handler: F) -> HsbResult<()>
    where
        F: Fn(KafkaMessage) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = HsbResult<()>> + Send + 'static,
    {
        self.subscribe(topics).await?;

        let consumer = self.consumer().await?;
        let stats = self.stats.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            let mut stream = consumer.stream();
            while let Some(message_result) = stream.next().await {
                match message_result {
                    Ok(message) => {
                        let kafka_message = KafkaMessage {
                            topic: message.topic().to_string(),
                            key: message.key().map(|k| k.to_vec()),
                            payload: Bytes::copy_from_slice(message.payload().unwrap_or_default()),
                            partition: message.partition(),
                            offset: message.offset(),
                            timestamp: message.timestamp().to_millis(),
                            headers: copy_headers(message.headers()),
                        };

                        match handler(kafka_message.clone()).await {
                            Ok(()) => {
                                stats.messages_received.fetch_add(1, Ordering::Relaxed);
                                stats.bytes_received.fetch_add(
                                    kafka_message.payload.len() as u64,
                                    Ordering::Relaxed,
                                );
                                if let Err(e) = consumer.commit_message(&message, CommitMode::Async)
                                {
                                    error!("Kafka commit failed: {}", e);
                                    stats.errors.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Err(e) => {
                                error!("Kafka message processing failed: {}", e);
                                stats.errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Kafka consume error: {}", e);
                        stats.errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });

        Ok(())
    }

    async fn producer(&self) -> HsbResult<FutureProducer> {
        let guard = self.producer.read().await;
        guard.clone().ok_or_else(|| HsbError::TransportError {
            message: "Kafka producer not connected".to_string(),
        })
    }

    async fn consumer(&self) -> HsbResult<Arc<StreamConsumer>> {
        let guard = self.consumer.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| HsbError::TransportError {
                message: "Kafka consumer not connected".to_string(),
            })
    }
}

#[async_trait]
impl Transport for KafkaTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::Mq
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
        let start = Instant::now();
        let topic = parse_kafka_target(&request.target, &self.config.default_topic)?;
        let headers = request
            .headers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();

        self.publish(&topic, request.body.clone(), None, Some(&headers))
            .await?;
        Ok(TransportResponse::success(Bytes::new(), start.elapsed()))
    }

    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        timeout: Duration,
    ) -> HsbResult<TransportResponse> {
        tokio::time::timeout(timeout, self.send(request))
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "Kafka send".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })?
    }

    async fn health_check(&self) -> HsbResult<HealthStatus> {
        if !self.connected.load(Ordering::Relaxed) {
            return Ok(HealthStatus::unhealthy("Not connected to Kafka"));
        }

        let producer = self.producer().await?;
        let metadata = producer
            .client()
            .fetch_metadata(None, Duration::from_secs(5))
            .map_err(|e| HsbError::TransportError {
                message: format!("Kafka metadata request failed: {}", e),
            })?;

        Ok(
            HealthStatus::healthy()
                .with_detail("brokers", &format!("{}", metadata.brokers().len())),
        )
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
impl ConnectableTransport for KafkaTransport {
    async fn connect(&self) -> HsbResult<()> {
        let producer = build_producer(&self.config)?;
        let consumer = build_consumer(&self.config)?;

        {
            let mut guard = self.producer.write().await;
            *guard = Some(producer);
        }
        {
            let mut guard = self.consumer.write().await;
            *guard = Some(Arc::new(consumer));
        }

        self.connected.store(true, Ordering::Relaxed);
        info!("Connected to Kafka at {}", self.config.bootstrap_servers);
        Ok(())
    }

    async fn disconnect(&self) -> HsbResult<()> {
        {
            let mut guard = self.producer.write().await;
            *guard = None;
        }
        {
            let mut guard = self.consumer.write().await;
            *guard = None;
        }
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

fn build_producer(config: &KafkaTransportConfig) -> HsbResult<FutureProducer> {
    let mut client = ClientConfig::new();
    apply_common_config(&mut client, config);
    client
        .set(
            "message.timeout.ms",
            (config.message_timeout_secs * 1000).to_string(),
        )
        .create()
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to create Kafka producer: {}", e),
        })
}

fn build_consumer(config: &KafkaTransportConfig) -> HsbResult<StreamConsumer> {
    let mut client = ClientConfig::new();
    apply_common_config(&mut client, config);
    client
        .set("group.id", &config.consumer.group_id)
        .set("enable.auto.commit", "false")
        .set(
            "session.timeout.ms",
            (config.consumer.session_timeout_secs * 1000).to_string(),
        )
        .set(
            "auto.offset.reset",
            if config.consumer.start_from_earliest {
                "earliest"
            } else {
                "latest"
            },
        )
        .create()
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to create Kafka consumer: {}", e),
        })
}

fn apply_common_config(client: &mut ClientConfig, config: &KafkaTransportConfig) {
    client
        .set("bootstrap.servers", &config.bootstrap_servers)
        .set("client.id", &config.client_id)
        .set("security.protocol", &config.security_protocol)
        .set(
            "socket.timeout.ms",
            (config.socket_timeout_secs * 1000).to_string(),
        );

    if let Some(username) = &config.sasl_username {
        client.set("sasl.username", username);
    }
    if let Some(password) = &config.sasl_password {
        client.set("sasl.password", password);
    }
    if let Some(mechanism) = &config.sasl_mechanism {
        client.set("sasl.mechanism", mechanism);
    }
}

fn parse_kafka_target(target: &str, default_topic: &Option<String>) -> HsbResult<String> {
    if let Some(topic) = target.strip_prefix("kafka://") {
        return Ok(topic.to_string());
    }

    if !target.is_empty() {
        return Ok(target.to_string());
    }

    default_topic.clone().ok_or_else(|| HsbError::ConfigError {
        message: "Kafka target topic is empty and no default topic configured".to_string(),
    })
}

fn copy_headers(headers: Option<&BorrowedHeaders>) -> Vec<(String, Vec<u8>)> {
    headers
        .map(|headers| {
            (0..headers.count())
                .map(|idx| headers.get(idx))
                .map(|header| {
                    (
                        header.key.to_string(),
                        header.value.unwrap_or_default().to_vec(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::parse_kafka_target;

    #[test]
    fn parse_kafka_target_prefers_scheme() {
        let topic = parse_kafka_target("kafka://medical.order.create.v1", &None)
            .expect("topic should parse");
        assert_eq!(topic, "medical.order.create.v1");
    }

    #[test]
    fn parse_kafka_target_falls_back_to_default() {
        let topic = parse_kafka_target("", &Some("hsb.default".to_string()))
            .expect("default topic should be used");
        assert_eq!(topic, "hsb.default");
    }
}
