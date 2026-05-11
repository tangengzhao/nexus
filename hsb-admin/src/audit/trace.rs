//! 消息追踪

use hsb_common::{ProtocolType, TraceId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{AuditEvent, AuditEventType};

/// 消息追踪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTrace {
    /// 追踪 ID
    pub trace_id: TraceId,
    /// 消息 ID
    pub message_id: String,
    /// 父追踪 ID（用于消息拆分场景）
    pub parent_trace_id: Option<TraceId>,
    /// 源系统
    pub source_system: String,
    /// 目标系统列表
    pub target_systems: Vec<String>,
    /// 协议
    pub protocol: ProtocolType,
    /// 消息类型
    pub message_type: Option<String>,
    /// 开始时间
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 总耗时（毫秒）
    pub total_duration_ms: Option<u64>,
    /// 状态
    pub status: TraceStatus,
    /// 追踪跨度列表
    pub spans: Vec<TraceSpan>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// 追踪状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceStatus {
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 部分成功
    PartialSuccess,
    /// 失败
    Failed,
}

/// 追踪跨度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSpan {
    /// 跨度 ID
    pub span_id: String,
    /// 父跨度 ID
    pub parent_span_id: Option<String>,
    /// 操作名称
    pub operation: String,
    /// 组件
    pub component: String,
    /// 开始时间
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// 结束时间
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// 状态
    pub status: SpanStatus,
    /// 错误信息
    pub error: Option<String>,
    /// 标签
    pub tags: HashMap<String, String>,
    /// 日志
    pub logs: Vec<SpanLog>,
}

/// 跨度状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    Ok,
    Error,
    Timeout,
}

/// 跨度日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanLog {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub message: String,
    pub fields: HashMap<String, String>,
}

impl MessageTrace {
    pub fn new(message_id: &str, source_system: &str, protocol: ProtocolType) -> Self {
        Self {
            trace_id: TraceId::new(),
            message_id: message_id.to_string(),
            parent_trace_id: None,
            source_system: source_system.to_string(),
            target_systems: Vec::new(),
            protocol,
            message_type: None,
            start_time: chrono::Utc::now(),
            end_time: None,
            total_duration_ms: None,
            status: TraceStatus::InProgress,
            spans: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// 添加跨度
    pub fn add_span(&mut self, span: TraceSpan) {
        self.spans.push(span);
    }

    /// 完成追踪
    pub fn complete(&mut self, success: bool) {
        let now = chrono::Utc::now();
        self.end_time = Some(now);
        self.total_duration_ms = Some((now - self.start_time).num_milliseconds().max(0) as u64);

        self.status = if success {
            TraceStatus::Completed
        } else {
            // 检查是否有部分成功
            let has_success = self.spans.iter().any(|s| s.status == SpanStatus::Ok);
            let has_error = self.spans.iter().any(|s| s.status == SpanStatus::Error);

            if has_success && has_error {
                TraceStatus::PartialSuccess
            } else {
                TraceStatus::Failed
            }
        };
    }

    /// 从审计事件列表构建追踪
    pub fn from_audit_events(events: &[AuditEvent]) -> Option<Self> {
        if events.is_empty() {
            return None;
        }

        // 找到第一个事件
        let first_event = events
            .iter()
            .filter(|e| e.event_type == AuditEventType::MessageReceived)
            .min_by_key(|e| e.timestamp)?;

        let trace_id = first_event.trace_id.clone()?;
        let message_id = first_event.message_id.as_ref()?;
        let source_system = first_event.source_system.as_ref()?;

        let mut trace = Self {
            trace_id,
            message_id: message_id.clone(),
            parent_trace_id: None,
            source_system: source_system.clone(),
            target_systems: Vec::new(),
            protocol: ProtocolType::Custom,
            message_type: None,
            start_time: first_event.timestamp,
            end_time: None,
            total_duration_ms: None,
            status: TraceStatus::InProgress,
            spans: Vec::new(),
            metadata: HashMap::new(),
        };

        // 构建跨度
        for event in events {
            let span = TraceSpan {
                span_id: event.id.clone(),
                parent_span_id: None,
                operation: event.event_type.as_str().to_string(),
                component: event.component.clone(),
                start_time: event.timestamp,
                end_time: event
                    .duration_ms
                    .map(|d| event.timestamp + chrono::Duration::milliseconds(d as i64)),
                duration_ms: event.duration_ms,
                status: if event.success {
                    SpanStatus::Ok
                } else {
                    SpanStatus::Error
                },
                error: event.error.clone(),
                tags: HashMap::new(),
                logs: Vec::new(),
            };
            trace.spans.push(span);

            // 收集目标系统
            if let Some(ref target) = event.target_system {
                if !trace.target_systems.contains(target) {
                    trace.target_systems.push(target.clone());
                }
            }
        }

        // 计算最终状态
        let last_event = events.iter().max_by_key(|e| e.timestamp)?;
        trace.end_time = Some(last_event.timestamp);
        trace.total_duration_ms = Some(
            (last_event.timestamp - trace.start_time)
                .num_milliseconds()
                .max(0) as u64,
        );

        let all_success = events.iter().all(|e| e.success);
        let any_success = events.iter().any(|e| e.success);
        trace.status = if all_success {
            TraceStatus::Completed
        } else if any_success {
            TraceStatus::PartialSuccess
        } else {
            TraceStatus::Failed
        };

        Some(trace)
    }
}

impl TraceSpan {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            span_id: ulid::Ulid::new().to_string(),
            parent_span_id: None,
            operation: operation.to_string(),
            component: component.to_string(),
            start_time: chrono::Utc::now(),
            end_time: None,
            duration_ms: None,
            status: SpanStatus::Ok,
            error: None,
            tags: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn with_parent(mut self, parent_id: &str) -> Self {
        self.parent_span_id = Some(parent_id.to_string());
        self
    }

    pub fn with_tag(mut self, key: &str, value: &str) -> Self {
        self.tags.insert(key.to_string(), value.to_string());
        self
    }

    pub fn add_log(&mut self, message: &str) {
        self.logs.push(SpanLog {
            timestamp: chrono::Utc::now(),
            message: message.to_string(),
            fields: HashMap::new(),
        });
    }

    pub fn finish(&mut self, success: bool, error: Option<&str>) {
        let now = chrono::Utc::now();
        self.end_time = Some(now);
        self.duration_ms = Some((now - self.start_time).num_milliseconds().max(0) as u64);
        self.status = if success {
            SpanStatus::Ok
        } else {
            SpanStatus::Error
        };
        self.error = error.map(String::from);
    }
}

/// 追踪上下文（用于跨服务传播）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: TraceId,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub sampled: bool,
}

impl TraceContext {
    pub fn new(trace_id: TraceId) -> Self {
        Self {
            trace_id,
            span_id: ulid::Ulid::new().to_string(),
            parent_span_id: None,
            sampled: true,
        }
    }

    /// 创建子上下文
    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: ulid::Ulid::new().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            sampled: self.sampled,
        }
    }

    /// 序列化为 HTTP 头格式
    pub fn to_header(&self) -> String {
        format!(
            "00-{}-{}-{}",
            self.trace_id,
            self.span_id,
            if self.sampled { "01" } else { "00" }
        )
    }

    /// 从 HTTP 头解析
    pub fn from_header(header: &str) -> Option<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() < 4 {
            return None;
        }

        Some(Self {
            trace_id: parts[1].parse().ok()?,
            span_id: parts[2].to_string(),
            parent_span_id: None,
            sampled: parts[3] == "01",
        })
    }
}
