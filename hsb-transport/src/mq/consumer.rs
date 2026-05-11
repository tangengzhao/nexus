//! 消息队列消费者

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use lapin::{Channel, message::Delivery, options::*, types::FieldTable};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

/// 消息处理器
pub trait MessageProcessor: Send + Sync {
    /// 处理消息
    fn process(
        &self,
        message: ConsumedMessage,
    ) -> impl std::future::Future<Output = ProcessResult> + Send;
}

/// 处理结果
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProcessResult {
    /// 确认消息
    Ack,
    /// 拒绝消息（重新入队）
    Nack,
    /// 拒绝消息（不重新入队）
    Reject,
}

/// 已消费的消息
#[derive(Debug, Clone)]
pub struct ConsumedMessage {
    /// 消息体
    pub body: Bytes,
    /// 消息 ID
    pub message_id: Option<String>,
    /// 关联 ID
    pub correlation_id: Option<String>,
    /// 回复队列
    pub reply_to: Option<String>,
    /// 路由键
    pub routing_key: String,
    /// Exchange
    pub exchange: String,
    /// 投递标签
    pub delivery_tag: u64,
    /// 是否重新投递
    pub redelivered: bool,
}

impl From<Delivery> for ConsumedMessage {
    fn from(delivery: Delivery) -> Self {
        Self {
            body: Bytes::from(delivery.data),
            message_id: delivery
                .properties
                .message_id()
                .as_ref()
                .map(|s| s.to_string()),
            correlation_id: delivery
                .properties
                .correlation_id()
                .as_ref()
                .map(|s| s.to_string()),
            reply_to: delivery
                .properties
                .reply_to()
                .as_ref()
                .map(|s| s.to_string()),
            routing_key: delivery.routing_key.to_string(),
            exchange: delivery.exchange.to_string(),
            delivery_tag: delivery.delivery_tag,
            redelivered: delivery.redelivered,
        }
    }
}

/// 消息队列消费者
pub struct MqConsumer {
    channel: Channel,
    queue_name: String,
    consumer_tag: String,
}

impl MqConsumer {
    pub fn new(channel: Channel, queue_name: &str) -> Self {
        Self {
            channel,
            queue_name: queue_name.to_string(),
            consumer_tag: format!("hsb-consumer-{}", ulid::Ulid::new()),
        }
    }

    /// 开始消费
    pub async fn start<P: MessageProcessor + 'static>(&self, processor: Arc<P>) -> HsbResult<()> {
        let consumer = self
            .channel
            .basic_consume(
                &self.queue_name,
                &self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to start consumer: {}", e),
            })?;

        info!("Started consuming from queue: {}", self.queue_name);

        let channel = self.channel.clone();

        tokio::spawn(async move {
            use futures_lite::stream::StreamExt;

            let mut consumer = consumer;
            while let Some(delivery_result) = consumer.next().await {
                match delivery_result {
                    Ok(delivery) => {
                        let delivery_tag = delivery.delivery_tag;
                        let message = ConsumedMessage::from(delivery);

                        let result = processor.process(message).await;

                        match result {
                            ProcessResult::Ack => {
                                if let Err(e) = channel
                                    .basic_ack(delivery_tag, BasicAckOptions::default())
                                    .await
                                {
                                    error!("Failed to ack message: {}", e);
                                }
                            }
                            ProcessResult::Nack => {
                                if let Err(e) = channel
                                    .basic_nack(
                                        delivery_tag,
                                        BasicNackOptions {
                                            multiple: false,
                                            requeue: true,
                                        },
                                    )
                                    .await
                                {
                                    error!("Failed to nack message: {}", e);
                                }
                            }
                            ProcessResult::Reject => {
                                if let Err(e) = channel
                                    .basic_reject(
                                        delivery_tag,
                                        BasicRejectOptions { requeue: false },
                                    )
                                    .await
                                {
                                    error!("Failed to reject message: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Consumer error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// 使用 channel 接收消息
    pub async fn consume_with_channel(&self) -> HsbResult<mpsc::Receiver<ConsumedMessage>> {
        let (tx, rx) = mpsc::channel(100);

        let consumer = self
            .channel
            .basic_consume(
                &self.queue_name,
                &self.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to start consumer: {}", e),
            })?;

        let channel = self.channel.clone();

        tokio::spawn(async move {
            use futures_lite::stream::StreamExt;

            let mut consumer = consumer;
            while let Some(delivery_result) = consumer.next().await {
                match delivery_result {
                    Ok(delivery) => {
                        let delivery_tag = delivery.delivery_tag;
                        let message = ConsumedMessage::from(delivery);

                        if tx.send(message).await.is_err() {
                            break;
                        }

                        // 自动确认
                        channel
                            .basic_ack(delivery_tag, BasicAckOptions::default())
                            .await
                            .ok();
                    }
                    Err(e) => {
                        error!("Consumer error: {}", e);
                    }
                }
            }
        });

        Ok(rx)
    }

    /// 停止消费
    pub async fn stop(&self) -> HsbResult<()> {
        self.channel
            .basic_cancel(&self.consumer_tag, BasicCancelOptions::default())
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to cancel consumer: {}", e),
            })?;

        info!("Stopped consuming from queue: {}", self.queue_name);
        Ok(())
    }
}
