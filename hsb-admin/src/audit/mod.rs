//! HSB 审计层
//!
//! 提供消息追踪、审计日志和合规性记录功能。

mod audit_log;
mod metrics;
mod storage;
mod trace;

pub use audit_log::*;
pub use metrics::*;
pub use storage::*;
pub use trace::*;

use async_trait::async_trait;
use hsb_common::HsbResult;
use serde::{Deserialize, Serialize};

/// 审计事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// 消息接收
    MessageReceived,
    /// 消息解析
    MessageParsed,
    /// 消息验证
    MessageValidated,
    /// 消息转换
    MessageTransformed,
    /// 消息路由
    MessageRouted,
    /// 消息发送
    MessageSent,
    /// 消息确认
    MessageAcknowledged,
    /// 消息失败
    MessageFailed,
    /// 消息重试
    MessageRetried,
    /// 消息死信
    MessageDeadLettered,
    /// 系统事件
    SystemEvent,
    /// 配置变更
    ConfigChanged,
    /// 安全事件
    SecurityEvent,
}

impl AuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MessageReceived => "MESSAGE_RECEIVED",
            Self::MessageParsed => "MESSAGE_PARSED",
            Self::MessageValidated => "MESSAGE_VALIDATED",
            Self::MessageTransformed => "MESSAGE_TRANSFORMED",
            Self::MessageRouted => "MESSAGE_ROUTED",
            Self::MessageSent => "MESSAGE_SENT",
            Self::MessageAcknowledged => "MESSAGE_ACKNOWLEDGED",
            Self::MessageFailed => "MESSAGE_FAILED",
            Self::MessageRetried => "MESSAGE_RETRIED",
            Self::MessageDeadLettered => "MESSAGE_DEAD_LETTERED",
            Self::SystemEvent => "SYSTEM_EVENT",
            Self::ConfigChanged => "CONFIG_CHANGED",
            Self::SecurityEvent => "SECURITY_EVENT",
        }
    }
}

/// 审计严重级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AuditSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// 审计服务 Trait
#[async_trait]
pub trait AuditService: Send + Sync {
    /// 记录审计事件
    async fn log_event(&self, event: AuditEvent) -> HsbResult<()>;

    /// 批量记录
    async fn log_events(&self, events: Vec<AuditEvent>) -> HsbResult<()>;

    /// 查询审计日志
    async fn query(&self, filter: AuditFilter) -> HsbResult<Vec<AuditEvent>>;

    /// 获取消息完整追踪
    async fn get_message_trace(&self, message_id: &str) -> HsbResult<MessageTrace>;
}

/// 审计配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    /// 是否启用审计
    pub enabled: bool,
    /// 最小严重级别
    pub min_severity: AuditSeverity,
    /// 是否记录消息内容
    pub log_message_content: bool,
    /// 是否脱敏
    pub mask_sensitive_data: bool,
    /// 敏感字段列表
    pub sensitive_fields: Vec<String>,
    /// 保留天数
    pub retention_days: u32,
    /// 批量写入大小
    pub batch_size: usize,
    /// 异步写入
    pub async_write: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_severity: AuditSeverity::Info,
            log_message_content: false,
            mask_sensitive_data: true,
            sensitive_fields: vec![
                "patient_name".to_string(),
                "patient_id".to_string(),
                "ssn".to_string(),
                "address".to_string(),
                "phone".to_string(),
            ],
            retention_days: 90,
            batch_size: 100,
            async_write: true,
        }
    }
}
