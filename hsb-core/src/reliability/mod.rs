//! HSB 可靠性层
//!
//! 提供消息队列、重试机制和死信队列功能。

mod circuit_breaker;
mod dlq;
mod queue;
mod retry;

pub use circuit_breaker::*;
pub use dlq::*;
pub use queue::*;
pub use retry::*;

use crate::Message;
use async_trait::async_trait;
use hsb_common::HsbResult;
use serde::{Deserialize, Serialize};

/// 消息存储 Trait
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// 保存消息
    async fn save(&self, msg: &Message) -> HsbResult<()>;

    /// 获取消息
    async fn get(&self, id: &str) -> HsbResult<Option<Message>>;

    /// 删除消息
    async fn delete(&self, id: &str) -> HsbResult<()>;

    /// 更新消息状态
    async fn update_status(&self, id: &str, status: MessageStatus) -> HsbResult<()>;

    /// 查询待处理消息
    async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>>;
}

/// 消息状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// 待处理
    Pending,
    /// 处理中
    Processing,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已重试
    Retrying,
    /// 在死信队列
    DeadLettered,
}

/// 消息元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMeta {
    /// 消息 ID
    pub message_id: String,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 状态
    pub status: MessageStatus,
    /// 重试次数
    pub retry_count: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 下次重试时间
    pub next_retry_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 最后错误
    pub last_error: Option<String>,
    /// 源系统
    pub source_system: String,
    /// 目标系统
    pub target_system: Option<String>,
    /// 消息类型
    pub message_type: Option<String>,
    /// 优先级
    pub priority: i32,
}

impl MessageMeta {
    pub fn new(msg: &Message) -> Self {
        let now = chrono::Utc::now();
        Self {
            message_id: msg.id.to_string(),
            created_at: now,
            updated_at: now,
            status: MessageStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            next_retry_at: None,
            last_error: None,
            source_system: msg.source_system.to_string(),
            target_system: msg.target_system.as_ref().map(|s| s.to_string()),
            message_type: msg.message_type.clone(),
            priority: 0,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn should_retry_now(&self) -> bool {
        if !self.can_retry() {
            return false;
        }

        match self.next_retry_at {
            Some(next) => chrono::Utc::now() >= next,
            None => true,
        }
    }
}

/// 可靠性层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilityConfig {
    /// 内存队列最大大小
    pub max_queue_size: usize,
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试基础延迟（毫秒）
    pub retry_base_delay_ms: u64,
    /// 重试最大延迟（毫秒）
    pub retry_max_delay_ms: u64,
    /// 是否启用死信队列
    pub enable_dlq: bool,
    /// 死信队列保留时间（天）
    pub dlq_retention_days: u32,
    /// 熔断器错误阈值
    pub circuit_breaker_threshold: u32,
    /// 熔断器恢复时间（秒）
    pub circuit_breaker_recovery_secs: u64,
}

impl Default for ReliabilityConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            max_retries: 3,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 300000, // 5 分钟
            enable_dlq: true,
            dlq_retention_days: 7,
            circuit_breaker_threshold: 5,
            circuit_breaker_recovery_secs: 60,
        }
    }
}
