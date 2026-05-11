//! 消息处理上下文
//!
//! 在消息处理过程中传递的上下文信息。

use crate::message::Message;
use chrono::{DateTime, Utc};
use hsb_common::TraceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use ulid::Ulid;

/// 消息处理上下文
///
/// 在消息从接收到投递的整个生命周期中携带上下文信息。
#[derive(Debug)]
pub struct MessageContext {
    /// 上下文 ID
    pub id: Ulid,

    /// 追踪 ID
    pub trace_id: TraceId,

    /// 关联的消息
    pub message: Arc<RwLock<Message>>,

    /// 处理开始时间
    pub started_at: DateTime<Utc>,

    /// 上下文属性
    pub attributes: Arc<RwLock<HashMap<String, ContextValue>>>,

    /// 处理链路记录
    pub processing_chain: Arc<RwLock<Vec<ProcessingRecord>>>,

    /// 用户信息（认证后填充）
    pub user: Option<UserInfo>,
}

/// 上下文属性值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Json(serde_json::Value),
}

impl From<String> for ContextValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for ContextValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for ContextValue {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<f64> for ContextValue {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for ContextValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

/// 处理记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingRecord {
    /// 处理阶段
    pub stage: ProcessingStage,
    /// 组件名称
    pub component: String,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 结束时间
    pub ended_at: Option<DateTime<Utc>>,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 附加信息
    pub details: HashMap<String, String>,
}

/// 处理阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessingStage {
    /// 接收
    Receive,
    /// 协议解析
    Parse,
    /// 路由匹配
    Route,
    /// 消息转换
    Transform,
    /// 消息投递
    Deliver,
    /// 确认
    Acknowledge,
    /// 重试
    Retry,
    /// 补偿
    Compensate,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// 用户 ID
    pub user_id: String,
    /// 用户名
    pub username: String,
    /// 角色列表
    pub roles: Vec<String>,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 租户 ID
    pub tenant_id: Option<String>,
}

impl MessageContext {
    /// 创建新的消息上下文
    pub fn new(message: Message) -> Self {
        let trace_id = message.trace_id.clone();
        Self {
            id: Ulid::new(),
            trace_id,
            message: Arc::new(RwLock::new(message)),
            started_at: Utc::now(),
            attributes: Arc::new(RwLock::new(HashMap::new())),
            processing_chain: Arc::new(RwLock::new(Vec::new())),
            user: None,
        }
    }

    /// 设置用户信息
    pub fn with_user(mut self, user: UserInfo) -> Self {
        self.user = Some(user);
        self
    }

    /// 设置属性
    pub async fn set_attribute(&self, key: impl Into<String>, value: impl Into<ContextValue>) {
        let mut attrs = self.attributes.write().await;
        attrs.insert(key.into(), value.into());
    }

    /// 获取属性
    pub async fn get_attribute(&self, key: &str) -> Option<ContextValue> {
        let attrs = self.attributes.read().await;
        attrs.get(key).cloned()
    }

    /// 记录处理开始
    pub async fn record_start(&self, stage: ProcessingStage, component: impl Into<String>) {
        let record = ProcessingRecord {
            stage,
            component: component.into(),
            started_at: Utc::now(),
            ended_at: None,
            success: false,
            error: None,
            details: HashMap::new(),
        };
        let mut chain = self.processing_chain.write().await;
        chain.push(record);
    }

    /// 记录处理完成
    pub async fn record_complete(
        &self,
        stage: ProcessingStage,
        success: bool,
        error: Option<String>,
    ) {
        let mut chain = self.processing_chain.write().await;
        if let Some(record) = chain
            .iter_mut()
            .rev()
            .find(|r| r.stage == stage && r.ended_at.is_none())
        {
            record.ended_at = Some(Utc::now());
            record.success = success;
            record.error = error;
        }
    }

    /// 获取处理耗时（毫秒）
    pub fn elapsed_ms(&self) -> i64 {
        (Utc::now() - self.started_at).num_milliseconds()
    }

    /// 获取消息的只读引用
    pub async fn message(&self) -> tokio::sync::RwLockReadGuard<'_, Message> {
        self.message.read().await
    }

    /// 获取消息的可写引用
    pub async fn message_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, Message> {
        self.message.write().await
    }

    /// 克隆为新上下文（用于分支处理）
    pub async fn fork(&self) -> Self {
        let message = self.message.read().await.clone();
        let attrs = self.attributes.read().await.clone();

        Self {
            id: Ulid::new(),
            trace_id: self.trace_id.clone(),
            message: Arc::new(RwLock::new(message)),
            started_at: Utc::now(),
            attributes: Arc::new(RwLock::new(attrs)),
            processing_chain: Arc::new(RwLock::new(Vec::new())),
            user: self.user.clone(),
        }
    }
}

impl Clone for MessageContext {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            trace_id: self.trace_id.clone(),
            message: Arc::clone(&self.message),
            started_at: self.started_at,
            attributes: Arc::clone(&self.attributes),
            processing_chain: Arc::clone(&self.processing_chain),
            user: self.user.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageBuilder;
    use hsb_common::ProtocolType;

    #[tokio::test]
    async fn test_message_context() {
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .raw_payload("{}")
            .build()
            .expect("Message should be valid");

        let ctx = MessageContext::new(msg);

        ctx.set_attribute("key1", "value1").await;
        ctx.record_start(ProcessingStage::Parse, "HL7Parser").await;

        let attr = ctx.get_attribute("key1").await;
        assert!(matches!(attr, Some(ContextValue::String(s)) if s == "value1"));
    }
}
