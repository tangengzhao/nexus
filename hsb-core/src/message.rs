//! 核心消息对象定义
//!
//! Message 是 HSB 内部的统一消息抽象，所有协议的数据都转换为此格式。
//!
//! JSON 线上格式约定：
//! ```json
//! {
//!   "id": "01HXXXXXX...",
//!   "topic": "medical.order.create.v1",
//!   "timestamp": 1710000000,
//!   "headers": { "trace_id": "xxx", "source": "his", "priority": "high" },
//!   "payload": {},
//!   "meta": { "retry_count": 0, ... }
//! }
//! ```

use chrono::{DateTime, Utc};
use hsb_common::{
    HsbError, HsbResult, MessagePriority, MessageStatus, ProtocolType, SystemId, Topic, TraceId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ulid::Ulid;

/// 核心消息对象
///
/// 所有医疗系统间的数据交换都抽象为 Message 对象。
/// 原始报文保存在 `raw_payload` 中，不可变。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息唯一标识（ULID，时间排序）
    pub id: Ulid,

    /// Topic —— 格式 `<domain>.<service>.<action>.<version>`
    /// 示例：`medical.order.create.v1`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<Topic>,

    /// Unix 时间戳（秒），与线上 JSON `timestamp` 字段对应
    pub timestamp: i64,

    /// 消息头部 —— 必须包含 `trace_id`、`source`、`priority`
    pub headers: HashMap<String, String>,

    /// 解析后的消息体（JSON 格式）
    pub payload: Option<serde_json::Value>,

    /// 消息运行时元信息（重试、状态等）
    pub meta: MessageMeta,

    // ---- 以下为内部扩展字段，线上传输时可忽略 ----
    /// 消息版本（乐观锁）
    pub version: u32,

    /// 源系统标识
    pub source_system: SystemId,

    /// 目标系统标识（可为空，由路由决定）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_system: Option<SystemId>,

    /// 协议类型
    pub protocol: ProtocolType,

    /// 消息类型（如 HL7 的 ADT^A01）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,

    /// 原始报文（不可变）
    #[serde(with = "serde_bytes")]
    pub raw_payload: Vec<u8>,

    /// 消息状态
    pub status: MessageStatus,

    /// 消息优先级
    pub priority: MessagePriority,

    /// 追踪标识
    pub trace_id: TraceId,

    /// 关联标识（用于请求-响应配对）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,

    /// 领域元数据（患者、科室等业务字段）
    pub metadata: MessageMetadata,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 消息运行时元信息
///
/// 对应 JSON `meta` 字段，用于重试控制、调度等。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMeta {
    /// 已重试次数
    pub retry_count: u32,

    /// 最大重试次数（0 = 不重试）
    #[serde(default)]
    pub max_retries: u32,

    /// 上次重试时间
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_retry_at: Option<DateTime<Utc>>,

    /// 死信原因
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dead_letter_reason: Option<String>,
}

/// 消息领域元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// 医院/分院 id
    pub hospital_id: Option<String>,
    /// 患者 ID
    pub patient_id: Option<String>,

    /// 就诊号
    pub visit_id: Option<String>,

    /// 医嘱号
    pub order_id: Option<String>,

    /// 科室代码
    pub department_code: Option<String>,

    /// 发送应用
    pub sending_application: Option<String>,

    /// 发送设施
    pub sending_facility: Option<String>,

    /// 接收应用
    pub receiving_application: Option<String>,

    /// 接收设施
    pub receiving_facility: Option<String>,

    /// 消息控制 ID（HL7 MSH-10）
    pub message_control_id: Option<String>,

    /// 处理 ID（HL7 MSH-11）
    pub processing_id: Option<String>,

    /// 自定义属性
    pub custom: HashMap<String, serde_json::Value>,
}

impl Message {
    /// 创建新消息
    ///
    /// 自动将 `trace_id`、`source`、`priority` 写入 headers。
    pub fn new(source_system: SystemId, protocol: ProtocolType, raw_payload: Vec<u8>) -> Self {
        let now = Utc::now();
        let trace_id = TraceId::new();
        let priority = MessagePriority::Normal;

        let mut headers = HashMap::new();
        headers.insert("trace_id".to_string(), trace_id.to_string());
        headers.insert("source".to_string(), source_system.to_string());
        headers.insert("priority".to_string(), priority.as_str().to_string());

        Self {
            id: Ulid::new(),
            topic: None,
            timestamp: now.timestamp(),
            headers,
            payload: None,
            meta: MessageMeta::default(),
            version: 1,
            source_system,
            target_system: None,
            protocol,
            message_type: None,
            raw_payload,
            status: MessageStatus::Received,
            priority,
            trace_id,
            correlation_id: None,
            metadata: MessageMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 设置 topic
    pub fn with_topic(mut self, topic: Topic) -> Self {
        self.topic = Some(topic);
        self
    }

    /// 设置目标系统
    pub fn with_target(mut self, target: SystemId) -> Self {
        self.target_system = Some(target);
        self
    }

    /// 设置消息类型
    pub fn with_message_type(mut self, msg_type: impl Into<String>) -> Self {
        self.message_type = Some(msg_type.into());
        self
    }

    /// 设置优先级（同步更新 headers）
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self.headers
            .insert("priority".to_string(), priority.as_str().to_string());
        self
    }

    /// 设置追踪 ID（同步更新 headers）
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.headers
            .insert("trace_id".to_string(), trace_id.to_string());
        self.trace_id = trace_id;
        self
    }

    /// 添加 Header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// 设置解析后的 payload
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    /// 更新状态
    pub fn update_status(&mut self, status: MessageStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        self.version = self.version.saturating_add(1);
    }

    /// 递增重试计数
    pub fn increment_retry(&mut self) {
        self.meta.retry_count = self.meta.retry_count.saturating_add(1);
        self.meta.last_retry_at = Some(Utc::now());
        self.headers
            .insert("retry_count".to_string(), self.meta.retry_count.to_string());
    }

    /// 同步 headers（确保 trace_id / source / priority 始终一致）
    pub fn sync_headers(&mut self) {
        self.headers
            .insert("trace_id".to_string(), self.trace_id.to_string());
        self.headers
            .insert("source".to_string(), self.source_system.to_string());
        self.headers
            .insert("priority".to_string(), self.priority.as_str().to_string());
    }

    /// 获取 Header 值
    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    /// 获取 payload 中的字段
    pub fn get_payload_field(&self, path: &str) -> Option<&serde_json::Value> {
        self.payload.as_ref().and_then(|p| {
            let parts: Vec<&str> = path.split('.').collect();
            let mut current = p;
            for part in parts {
                current = current.get(part)?;
            }
            Some(current)
        })
    }

    /// 消息大小（字节）
    pub fn size(&self) -> usize {
        self.raw_payload.len()
    }

    /// 消息年龄
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// 是否为紧急消息
    pub fn is_urgent(&self) -> bool {
        matches!(
            self.priority,
            MessagePriority::Urgent | MessagePriority::Critical
        )
    }

    /// 是否已完成
    pub fn is_completed(&self) -> bool {
        self.status.is_terminal()
    }

    /// 克隆为响应消息
    pub fn clone_as_response(&self, response_payload: Vec<u8>) -> Self {
        let now = Utc::now();
        let source = self
            .target_system
            .clone()
            .unwrap_or_else(|| self.source_system.clone());

        let mut headers = HashMap::new();
        headers.insert("trace_id".to_string(), self.trace_id.to_string());
        headers.insert("source".to_string(), source.to_string());
        headers.insert("priority".to_string(), self.priority.as_str().to_string());

        Self {
            id: Ulid::new(),
            topic: None,
            timestamp: now.timestamp(),
            headers,
            payload: None,
            meta: MessageMeta::default(),
            version: 1,
            source_system: source,
            target_system: Some(self.source_system.clone()),
            protocol: self.protocol,
            message_type: None,
            raw_payload: response_payload,
            status: MessageStatus::Received,
            priority: self.priority,
            trace_id: self.trace_id.clone(),
            correlation_id: Some(self.id.to_string()),
            metadata: MessageMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// 消息构建器
#[derive(Debug, Default)]
pub struct MessageBuilder {
    source_system: Option<SystemId>,
    target_system: Option<SystemId>,
    protocol: Option<ProtocolType>,
    topic: Option<Topic>,
    message_type: Option<String>,
    headers: HashMap<String, String>,
    payload: Option<serde_json::Value>,
    raw_payload: Option<Vec<u8>>,
    priority: MessagePriority,
    trace_id: Option<TraceId>,
    correlation_id: Option<String>,
    metadata: MessageMetadata,
}

impl MessageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source_system(mut self, source: impl Into<SystemId>) -> Self {
        self.source_system = Some(source.into());
        self
    }

    pub fn target_system(mut self, target: impl Into<SystemId>) -> Self {
        self.target_system = Some(target.into());
        self
    }

    pub fn protocol(mut self, protocol: ProtocolType) -> Self {
        self.protocol = Some(protocol);
        self
    }

    pub fn topic(mut self, topic: Topic) -> Self {
        self.topic = Some(topic);
        self
    }

    pub fn message_type(mut self, msg_type: impl Into<String>) -> Self {
        self.message_type = Some(msg_type.into());
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn raw_payload(mut self, raw: impl Into<Vec<u8>>) -> Self {
        self.raw_payload = Some(raw.into());
        self
    }

    pub fn priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn patient_id(mut self, id: impl Into<String>) -> Self {
        self.metadata.patient_id = Some(id.into());
        self
    }

    pub fn visit_id(mut self, id: impl Into<String>) -> Self {
        self.metadata.visit_id = Some(id.into());
        self
    }

    /// 构建消息
    pub fn build(self) -> HsbResult<Message> {
        let source_system = self
            .source_system
            .ok_or_else(|| HsbError::ValidationError {
                message: "source_system is required".to_string(),
            })?;

        let protocol = self.protocol.ok_or_else(|| HsbError::ValidationError {
            message: "protocol is required".to_string(),
        })?;

        let raw_payload = self.raw_payload.unwrap_or_default();
        let now = Utc::now();
        let trace_id = self.trace_id.unwrap_or_default();

        // 确保 headers 包含必要字段
        let mut headers = self.headers;
        headers
            .entry("trace_id".to_string())
            .or_insert_with(|| trace_id.to_string());
        headers
            .entry("source".to_string())
            .or_insert_with(|| source_system.to_string());
        headers
            .entry("priority".to_string())
            .or_insert_with(|| self.priority.as_str().to_string());

        Ok(Message {
            id: Ulid::new(),
            topic: self.topic,
            timestamp: now.timestamp(),
            headers,
            payload: self.payload,
            meta: MessageMeta::default(),
            version: 1,
            source_system,
            target_system: self.target_system,
            protocol,
            message_type: self.message_type,
            raw_payload,
            status: MessageStatus::Received,
            priority: self.priority,
            trace_id,
            correlation_id: self.correlation_id,
            metadata: self.metadata,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_builder() {
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .target_system("LIS")
            .protocol(ProtocolType::Hl7V2)
            .topic(Topic::parse("medical.order.create.v1").unwrap())
            .message_type("ORM^O01")
            .priority(MessagePriority::High)
            .raw_payload("MSH|^~\\&|...")
            .build();

        assert!(msg.is_ok());
        let msg = msg.expect("Message should be valid");
        assert_eq!(msg.source_system.as_str(), "HIS");
        assert_eq!(msg.priority, MessagePriority::High);
        assert_eq!(
            msg.topic.as_ref().unwrap().as_str(),
            "medical.order.create.v1"
        );
        // headers 自动包含必要字段
        assert!(msg.headers.contains_key("trace_id"));
        assert_eq!(msg.headers.get("source").unwrap(), "HIS");
        assert_eq!(msg.headers.get("priority").unwrap(), "high");
        assert_eq!(msg.meta.retry_count, 0);
    }

    #[test]
    fn test_message_builder_missing_required() {
        let result = MessageBuilder::new().target_system("LIS").build();

        assert!(result.is_err());
    }

    #[test]
    fn test_topic_validation() {
        assert!(Topic::parse("medical.order.create.v1").is_ok());
        assert!(Topic::parse("ai.infer.request.v1").is_ok());
        assert!(Topic::parse("system.audit.log.v1").is_ok());
        // 不是 4 段
        assert!(Topic::parse("medical.order").is_err());
        // version 不以 v 开头
        assert!(Topic::parse("medical.order.create.1").is_err());
    }

    #[test]
    fn test_message_json_structure() {
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Hl7V2)
            .topic(Topic::parse("medical.order.create.v1").unwrap())
            .build()
            .unwrap();

        let json = serde_json::to_value(&msg).unwrap();
        // 顶层字段检查
        assert!(json.get("id").is_some());
        assert_eq!(json["topic"], "medical.order.create.v1");
        assert!(json.get("timestamp").is_some());
        assert!(json.get("headers").is_some());
        assert!(json.get("payload").is_some());
        assert!(json.get("meta").is_some());
        // headers 包含约定字段
        let hdrs = json["headers"].as_object().unwrap();
        assert!(hdrs.contains_key("trace_id"));
        assert!(hdrs.contains_key("source"));
        assert!(hdrs.contains_key("priority"));
        // meta 包含 retry_count
        assert_eq!(json["meta"]["retry_count"], 0);
    }
}
