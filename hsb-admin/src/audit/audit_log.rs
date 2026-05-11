//! 审计日志

use hsb_common::TraceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{AuditEventType, AuditSeverity};

/// 审计事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// 事件 ID
    pub id: String,
    /// 追踪 ID
    pub trace_id: Option<TraceId>,
    /// 消息 ID
    pub message_id: Option<String>,
    /// 事件类型
    pub event_type: AuditEventType,
    /// 严重级别
    pub severity: AuditSeverity,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 源系统
    pub source_system: Option<String>,
    /// 目标系统
    pub target_system: Option<String>,
    /// 处理器/组件名称
    pub component: String,
    /// 操作描述
    pub description: String,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
    /// 耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// 消息内容（可选，脱敏后）
    pub message_content: Option<String>,
    /// 额外元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 用户/客户端信息
    pub actor: Option<Actor>,
}

/// 操作者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    /// 用户 ID
    pub user_id: Option<String>,
    /// 用户名
    pub username: Option<String>,
    /// 客户端 IP
    pub client_ip: Option<String>,
    /// 客户端 ID
    pub client_id: Option<String>,
}

impl AuditEvent {
    pub fn new(event_type: AuditEventType, component: &str, description: &str) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            trace_id: None,
            message_id: None,
            event_type,
            severity: AuditSeverity::Info,
            timestamp: chrono::Utc::now(),
            source_system: None,
            target_system: None,
            component: component.to_string(),
            description: description.to_string(),
            success: true,
            error: None,
            duration_ms: None,
            message_content: None,
            metadata: HashMap::new(),
            actor: None,
        }
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_message_id(mut self, message_id: &str) -> Self {
        self.message_id = Some(message_id.to_string());
        self
    }

    pub fn with_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.source_system = Some(source.to_string());
        self
    }

    pub fn with_target(mut self, target: &str) -> Self {
        self.target_system = Some(target.to_string());
        self
    }

    pub fn with_error(mut self, error: &str) -> Self {
        self.success = false;
        self.error = Some(error.to_string());
        self.severity = AuditSeverity::Error;
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    pub fn with_actor(mut self, actor: Actor) -> Self {
        self.actor = Some(actor);
        self
    }
}

/// 审计事件构建器
pub struct AuditEventBuilder {
    event: AuditEvent,
}

impl AuditEventBuilder {
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            event: AuditEvent::new(event_type, "unknown", ""),
        }
    }

    pub fn component(mut self, component: &str) -> Self {
        self.event.component = component.to_string();
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.event.description = description.to_string();
        self
    }

    pub fn trace_id(mut self, trace_id: TraceId) -> Self {
        self.event.trace_id = Some(trace_id);
        self
    }

    pub fn message_id(mut self, message_id: &str) -> Self {
        self.event.message_id = Some(message_id.to_string());
        self
    }

    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.event.severity = severity;
        self
    }

    pub fn source(mut self, source: &str) -> Self {
        self.event.source_system = Some(source.to_string());
        self
    }

    pub fn target(mut self, target: &str) -> Self {
        self.event.target_system = Some(target.to_string());
        self
    }

    pub fn success(mut self) -> Self {
        self.event.success = true;
        self
    }

    pub fn failure(mut self, error: &str) -> Self {
        self.event.success = false;
        self.event.error = Some(error.to_string());
        self
    }

    pub fn duration(mut self, duration_ms: u64) -> Self {
        self.event.duration_ms = Some(duration_ms);
        self
    }

    pub fn metadata(mut self, key: &str, value: serde_json::Value) -> Self {
        self.event.metadata.insert(key.to_string(), value);
        self
    }

    pub fn actor(mut self, actor: Actor) -> Self {
        self.event.actor = Some(actor);
        self
    }

    pub fn build(self) -> AuditEvent {
        self.event
    }
}

/// 审计过滤器
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    /// 追踪 ID
    pub trace_id: Option<TraceId>,
    /// 消息 ID
    pub message_id: Option<String>,
    /// 事件类型
    pub event_types: Option<Vec<AuditEventType>>,
    /// 最小严重级别
    pub min_severity: Option<AuditSeverity>,
    /// 源系统
    pub source_system: Option<String>,
    /// 目标系统
    pub target_system: Option<String>,
    /// 组件
    pub component: Option<String>,
    /// 开始时间
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 结束时间
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 仅失败事件
    pub failed_only: bool,
    /// 限制
    pub limit: Option<usize>,
    /// 偏移
    pub offset: Option<usize>,
}

impl AuditFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn with_message_id(mut self, message_id: &str) -> Self {
        self.message_id = Some(message_id.to_string());
        self
    }

    pub fn with_event_types(mut self, types: Vec<AuditEventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    pub fn with_time_range(
        mut self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        self.from_time = Some(from);
        self.to_time = Some(to);
        self
    }

    pub fn failed_only(mut self) -> Self {
        self.failed_only = true;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}
