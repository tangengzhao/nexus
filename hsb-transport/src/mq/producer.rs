//! 消息队列生产者

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use lapin::{BasicProperties, Channel, options::*};
use std::collections::HashMap;

/// 消息队列生产者
pub struct MqProducer {
    channel: Channel,
}

impl MqProducer {
    pub fn new(channel: Channel) -> Self {
        Self { channel }
    }

    /// 发布消息
    pub async fn publish(
        &self,
        exchange: &str,
        routing_key: &str,
        message: Bytes,
        options: PublishOptions,
    ) -> HsbResult<()> {
        let mut properties = BasicProperties::default()
            .with_delivery_mode(if options.persistent { 2 } else { 1 })
            .with_content_type(options.content_type.clone().into());

        if let Some(ref message_id) = options.message_id {
            properties = properties.with_message_id(message_id.clone().into());
        }

        if let Some(ref correlation_id) = options.correlation_id {
            properties = properties.with_correlation_id(correlation_id.clone().into());
        }

        if let Some(ref reply_to) = options.reply_to {
            properties = properties.with_reply_to(reply_to.clone().into());
        }

        if let Some(expiration) = options.expiration_ms {
            properties = properties.with_expiration(expiration.to_string().into());
        }

        if let Some(priority) = options.priority {
            properties = properties.with_priority(priority);
        }

        self.channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions {
                    mandatory: options.mandatory,
                    immediate: false,
                },
                &message,
                properties,
            )
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to publish: {}", e),
            })?
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Publish confirmation failed: {}", e),
            })?;

        Ok(())
    }

    /// 发布到队列（使用默认 exchange）
    pub async fn publish_to_queue(&self, queue: &str, message: Bytes) -> HsbResult<()> {
        self.publish("", queue, message, PublishOptions::default())
            .await
    }

    /// 批量发布
    pub async fn publish_batch(
        &self,
        exchange: &str,
        messages: Vec<(String, Bytes)>, // (routing_key, message)
    ) -> HsbResult<usize> {
        let mut success_count = 0;

        for (routing_key, message) in messages {
            match self
                .publish(exchange, &routing_key, message, PublishOptions::default())
                .await
            {
                Ok(_) => success_count += 1,
                Err(e) => {
                    tracing::warn!("Failed to publish message: {}", e);
                }
            }
        }

        Ok(success_count)
    }
}

/// 发布选项
#[derive(Debug, Clone)]
pub struct PublishOptions {
    /// 是否持久化
    pub persistent: bool,
    /// 内容类型
    pub content_type: String,
    /// 消息 ID
    pub message_id: Option<String>,
    /// 关联 ID
    pub correlation_id: Option<String>,
    /// 回复队列
    pub reply_to: Option<String>,
    /// 过期时间（毫秒）
    pub expiration_ms: Option<u64>,
    /// 优先级 (0-9)
    pub priority: Option<u8>,
    /// 是否强制路由
    pub mandatory: bool,
    /// 自定义头
    pub headers: HashMap<String, String>,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            persistent: true,
            content_type: "application/octet-stream".to_string(),
            message_id: None,
            correlation_id: None,
            reply_to: None,
            expiration_ms: None,
            priority: None,
            mandatory: false,
            headers: HashMap::new(),
        }
    }
}

impl PublishOptions {
    pub fn json() -> Self {
        Self {
            content_type: "application/json".to_string(),
            ..Default::default()
        }
    }

    pub fn with_message_id(mut self, id: &str) -> Self {
        self.message_id = Some(id.to_string());
        self
    }

    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }

    pub fn with_reply_to(mut self, queue: &str) -> Self {
        self.reply_to = Some(queue.to_string());
        self
    }

    pub fn with_expiration(mut self, ms: u64) -> Self {
        self.expiration_ms = Some(ms);
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority.min(9));
        self
    }
}
