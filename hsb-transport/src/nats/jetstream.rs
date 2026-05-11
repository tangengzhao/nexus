//! JetStream 管理器
//!
//! 封装 JetStream 的流管理、消息发布和消费功能。

use async_nats::jetstream;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::stream::Stream;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use tracing::{info, warn};

use super::config::{JetStreamConfig, RetentionPolicy, StorageType};

/// JetStream 管理器
#[derive(Clone)]
pub struct JetStreamManager {
    context: jetstream::Context,
    config: JetStreamConfig,
}

impl JetStreamManager {
    pub fn new(context: jetstream::Context, config: JetStreamConfig) -> Self {
        Self { context, config }
    }

    /// 确保默认流存在
    pub async fn ensure_stream(&self) -> HsbResult<Stream> {
        let retention = match self.config.retention {
            RetentionPolicy::Limits => jetstream::stream::RetentionPolicy::Limits,
            RetentionPolicy::Interest => jetstream::stream::RetentionPolicy::Interest,
            RetentionPolicy::WorkQueue => jetstream::stream::RetentionPolicy::WorkQueue,
        };

        let storage = match self.config.storage {
            StorageType::File => jetstream::stream::StorageType::File,
            StorageType::Memory => jetstream::stream::StorageType::Memory,
        };

        let max_age = if self.config.max_age_secs > 0 {
            std::time::Duration::from_secs(self.config.max_age_secs)
        } else {
            std::time::Duration::ZERO
        };

        let stream = self
            .context
            .get_or_create_stream(jetstream::stream::Config {
                name: self.config.default_stream.clone(),
                subjects: self.config.stream_subjects.clone(),
                retention: retention,
                storage: storage,
                max_messages: self.config.max_messages,
                max_bytes: self.config.max_bytes,
                max_age: max_age,
                num_replicas: self.config.num_replicas,
                duplicate_window: std::time::Duration::from_secs(self.config.dedup_window_secs),
                ..Default::default()
            })
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to create/get JetStream stream: {}", e),
            })?;

        info!(
            "JetStream stream '{}' ready (subjects: {:?})",
            self.config.default_stream, self.config.stream_subjects
        );

        Ok(stream)
    }

    /// 发布消息到 JetStream（带持久化确认）
    pub async fn publish(&self, subject: &str, payload: Bytes) -> HsbResult<()> {
        let ack = self
            .context
            .publish(subject.to_string(), payload.into())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("JetStream publish failed: {}", e),
            })?
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("JetStream publish ack failed: {}", e),
            })?;

        if ack.duplicate {
            warn!(
                "JetStream duplicate message detected for subject {}",
                subject
            );
        }

        Ok(())
    }

    /// 发布消息到 JetStream（带幂等键，用于 ExactlyOnce 语义）
    pub async fn publish_with_dedup(
        &self,
        subject: &str,
        payload: Bytes,
        msg_id: &str,
    ) -> HsbResult<()> {
        let ack = self
            .context
            .publish_with_headers(
                subject.to_string(),
                {
                    let mut headers = async_nats::HeaderMap::new();
                    headers.insert(
                        "Nats-Msg-Id",
                        msg_id.parse::<async_nats::HeaderValue>().unwrap(),
                    );
                    headers
                },
                payload.into(),
            )
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("JetStream publish failed: {}", e),
            })?
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("JetStream publish ack failed: {}", e),
            })?;

        if ack.duplicate {
            info!(
                "JetStream dedup: message {} already delivered to {}",
                msg_id, subject
            );
        }

        Ok(())
    }

    /// 创建拉取式消费者
    pub async fn create_pull_consumer(
        &self,
        consumer_name: &str,
        filter_subject: Option<&str>,
    ) -> HsbResult<PullConsumer> {
        let stream = self.ensure_stream().await?;

        let config = jetstream::consumer::pull::Config {
            durable_name: Some(consumer_name.to_string()),
            filter_subject: filter_subject.unwrap_or_default().to_string(),
            ack_policy: jetstream::consumer::AckPolicy::Explicit,
            max_deliver: 3,
            ack_wait: std::time::Duration::from_secs(30),
            ..Default::default()
        };

        let consumer = stream
            .get_or_create_consumer(consumer_name, config)
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to create JetStream consumer: {}", e),
            })?;

        info!("JetStream pull consumer '{}' ready", consumer_name);
        Ok(consumer)
    }

    /// 获取 JetStream 上下文
    pub fn context(&self) -> &jetstream::Context {
        &self.context
    }
}
