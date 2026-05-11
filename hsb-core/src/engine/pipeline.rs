//! 消息处理管道

use crate::{MessageContext, ProcessingStage, Transformer, TransformerChain};
use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use super::MessageProcessor;

/// 处理管道
pub struct ProcessingPipeline {
    processors: Vec<Arc<dyn MessageProcessor>>,
    config: PipelineConfig,
}

impl ProcessingPipeline {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            config: PipelineConfig::default(),
        }
    }

    pub fn with_config(config: PipelineConfig) -> Self {
        Self {
            processors: Vec::new(),
            config,
        }
    }

    /// 添加处理器
    pub fn add_processor(&mut self, processor: Arc<dyn MessageProcessor>) {
        self.processors.push(processor);
        // 按优先级排序
        self.processors.sort_by_key(|p| p.priority());
    }

    /// 执行管道
    #[instrument(skip(self, ctx))]
    pub async fn execute(&self, ctx: &mut MessageContext) -> HsbResult<()> {
        for processor in &self.processors {
            let start = std::time::Instant::now();

            // 记录处理开始
            ctx.record_start(ProcessingStage::Transform, processor.name())
                .await;

            // 执行处理器
            let result = if self.config.enable_timeout {
                tokio::time::timeout(
                    std::time::Duration::from_secs(self.config.processor_timeout_secs),
                    processor.process(ctx),
                )
                .await
                .map_err(|_| HsbError::TimeoutError {
                    operation: format!("Processor: {}", processor.name()),
                    timeout_ms: self.config.processor_timeout_secs * 1000,
                })?
            } else {
                processor.process(ctx).await
            };

            let duration = start.elapsed();
            let _ = duration; // 记录用时

            // 更新处理记录
            ctx.record_complete(
                ProcessingStage::Transform,
                result.is_ok(),
                result.as_ref().err().map(|e| e.to_string()),
            )
            .await;

            // 处理错误
            match result {
                Ok(_) => {
                    info!("Processor {} completed in {:?}", processor.name(), duration);
                }
                Err(e) => {
                    error!("Processor {} failed: {}", processor.name(), e);

                    if self.config.fail_fast {
                        return Err(e);
                    } else {
                        warn!("Continuing pipeline despite error (fail_fast=false)");
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取处理器列表
    pub fn processors(&self) -> &[Arc<dyn MessageProcessor>] {
        &self.processors
    }
}

impl Default for ProcessingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// 管道配置
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// 处理器超时（秒）
    pub processor_timeout_secs: u64,
    /// 是否启用超时
    pub enable_timeout: bool,
    /// 是否快速失败
    pub fail_fast: bool,
    /// 最大处理器数量
    pub max_processors: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            processor_timeout_secs: 30,
            enable_timeout: true,
            fail_fast: true,
            max_processors: 100,
        }
    }
}

/// 转换处理器
pub struct TransformProcessor {
    chain: TransformerChain,
}

impl TransformProcessor {
    pub fn new(transformers: Vec<Arc<dyn Transformer>>) -> Self {
        let mut chain = TransformerChain::new();
        for transformer in transformers {
            chain = chain.add(transformer);
        }
        Self { chain }
    }
}

#[async_trait]
impl MessageProcessor for TransformProcessor {
    async fn process(&self, ctx: &mut MessageContext) -> HsbResult<()> {
        let msg = ctx.message().await.clone();
        let transformed_msg = self.chain.execute(msg).await?;
        *ctx.message_mut().await = transformed_msg;
        Ok(())
    }

    fn name(&self) -> &str {
        "TransformProcessor"
    }

    fn priority(&self) -> i32 {
        10 // 转换通常在早期执行
    }
}

/// 验证处理器
pub struct ValidationProcessor {
    strict: bool,
}

impl ValidationProcessor {
    pub fn new(strict: bool) -> Self {
        Self { strict }
    }
}

#[async_trait]
impl MessageProcessor for ValidationProcessor {
    async fn process(&self, ctx: &mut MessageContext) -> HsbResult<()> {
        let msg = ctx.message().await;

        // 基本验证
        if msg.source_system.as_str().is_empty() {
            if self.strict {
                return Err(HsbError::ValidationError {
                    message: "source_system is required".to_string(),
                });
            } else {
                warn!("source_system is empty");
            }
        }

        if msg.raw_payload.is_empty() && msg.payload.is_none() {
            if self.strict {
                return Err(HsbError::ValidationError {
                    message: "Message payload is empty".to_string(),
                });
            } else {
                warn!("Message payload is empty");
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "ValidationProcessor"
    }

    fn priority(&self) -> i32 {
        5 // 验证应该在最早期执行
    }
}

/// 日志处理器
pub struct LoggingProcessor {
    log_payload: bool,
}

impl LoggingProcessor {
    pub fn new(log_payload: bool) -> Self {
        Self { log_payload }
    }
}

#[async_trait]
impl MessageProcessor for LoggingProcessor {
    async fn process(&self, ctx: &mut MessageContext) -> HsbResult<()> {
        let msg = ctx.message().await;

        info!(
            message_id = %msg.id,
            source = %msg.source_system,
            protocol = ?msg.protocol,
            message_type = ?msg.message_type,
            "Processing message"
        );

        if self.log_payload {
            if let Some(ref payload) = msg.payload {
                info!(payload = %payload, "Message payload");
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "LoggingProcessor"
    }

    fn priority(&self) -> i32 {
        0 // 日志在最开始
    }
}

/// 指标收集处理器
pub struct MetricsProcessor;

#[async_trait]
impl MessageProcessor for MetricsProcessor {
    async fn process(&self, _ctx: &mut MessageContext) -> HsbResult<()> {
        // TODO: 收集指标（消息计数、大小、处理时间等）
        Ok(())
    }

    fn name(&self) -> &str {
        "MetricsProcessor"
    }

    fn priority(&self) -> i32 {
        1
    }
}
