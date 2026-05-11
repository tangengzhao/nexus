//! 审计存储

use async_trait::async_trait;
use hsb_common::HsbResult;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{AuditConfig, AuditEvent, AuditFilter, AuditService, MessageTrace};

/// 内存审计存储
pub struct InMemoryAuditStorage {
    events: Arc<RwLock<Vec<AuditEvent>>>,
    config: AuditConfig,
    max_events: usize,
}

impl InMemoryAuditStorage {
    pub fn new(config: AuditConfig, max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::with_capacity(max_events))),
            config,
            max_events,
        }
    }

    fn apply_filter<'a>(events: &'a [AuditEvent], filter: &AuditFilter) -> Vec<&'a AuditEvent> {
        events
            .iter()
            .filter(|e| {
                // Trace ID
                if let Some(ref trace_id) = filter.trace_id {
                    if e.trace_id.as_ref() != Some(trace_id) {
                        return false;
                    }
                }

                // Message ID
                if let Some(ref message_id) = filter.message_id {
                    if e.message_id.as_ref() != Some(message_id) {
                        return false;
                    }
                }

                // Event types
                if let Some(ref types) = filter.event_types {
                    if !types.contains(&e.event_type) {
                        return false;
                    }
                }

                // Min severity
                if let Some(min_severity) = filter.min_severity {
                    if e.severity < min_severity {
                        return false;
                    }
                }

                // Source system
                if let Some(ref source) = filter.source_system {
                    if e.source_system.as_ref() != Some(source) {
                        return false;
                    }
                }

                // Target system
                if let Some(ref target) = filter.target_system {
                    if e.target_system.as_ref() != Some(target) {
                        return false;
                    }
                }

                // Component
                if let Some(ref component) = filter.component {
                    if &e.component != component {
                        return false;
                    }
                }

                // Time range
                if let Some(from) = filter.from_time {
                    if e.timestamp < from {
                        return false;
                    }
                }
                if let Some(to) = filter.to_time {
                    if e.timestamp > to {
                        return false;
                    }
                }

                // Failed only
                if filter.failed_only && e.success {
                    return false;
                }

                true
            })
            .collect()
    }
}

#[async_trait]
impl AuditService for InMemoryAuditStorage {
    async fn log_event(&self, event: AuditEvent) -> HsbResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if event.severity < self.config.min_severity {
            return Ok(());
        }

        let mut events = self.events.write().await;

        // 如果超过最大容量，移除旧事件
        if events.len() >= self.max_events {
            events.remove(0);
        }

        events.push(event);
        Ok(())
    }

    async fn log_events(&self, events: Vec<AuditEvent>) -> HsbResult<()> {
        for event in events {
            self.log_event(event).await?;
        }
        Ok(())
    }

    async fn query(&self, filter: AuditFilter) -> HsbResult<Vec<AuditEvent>> {
        let events = self.events.read().await;
        let mut filtered: Vec<AuditEvent> = Self::apply_filter(&events, &filter)
            .into_iter()
            .cloned()
            .collect();

        // 按时间倒序
        filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // 应用分页
        let offset = filter.offset.unwrap_or(0);
        let limit = filter.limit.unwrap_or(100);

        Ok(filtered.into_iter().skip(offset).take(limit).collect())
    }

    async fn get_message_trace(&self, message_id: &str) -> HsbResult<MessageTrace> {
        let events = self.events.read().await;

        let message_events: Vec<AuditEvent> = events
            .iter()
            .filter(|e| {
                e.message_id
                    .as_ref()
                    .map(|id| id == message_id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        MessageTrace::from_audit_events(&message_events).ok_or_else(|| {
            hsb_common::HsbError::NotFound {
                entity: "MessageTrace".to_string(),
                id: message_id.to_string(),
            }
        })
    }
}

/// 敏感数据脱敏器
pub struct DataMasker {
    patterns: Vec<MaskPattern>,
}

impl DataMasker {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn with_default_patterns() -> Self {
        let mut masker = Self::new();

        // 添加默认脱敏规则
        masker.add_pattern(MaskPattern::field(
            "patient_id",
            MaskType::Partial { visible_chars: 4 },
        ));
        masker.add_pattern(MaskPattern::field("patient_name", MaskType::Full));
        masker.add_pattern(MaskPattern::field(
            "ssn",
            MaskType::Partial { visible_chars: 4 },
        ));
        masker.add_pattern(MaskPattern::field(
            "phone",
            MaskType::Partial { visible_chars: 4 },
        ));
        masker.add_pattern(MaskPattern::field("address", MaskType::Full));
        masker.add_pattern(MaskPattern::field("email", MaskType::Email));
        masker.add_pattern(MaskPattern::field("date_of_birth", MaskType::Full));

        masker
    }

    pub fn add_pattern(&mut self, pattern: MaskPattern) {
        self.patterns.push(pattern);
    }

    /// 脱敏 JSON
    pub fn mask_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (key, val) in map {
                    let masked = if let Some(pattern) = self.find_pattern(key) {
                        pattern.mask(val)
                    } else {
                        self.mask_json(val)
                    };
                    new_map.insert(key.clone(), masked);
                }
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.mask_json(v)).collect())
            }
            _ => value.clone(),
        }
    }

    fn find_pattern(&self, field: &str) -> Option<&MaskPattern> {
        self.patterns.iter().find(|p| p.matches(field))
    }
}

impl Default for DataMasker {
    fn default() -> Self {
        Self::new()
    }
}

/// 脱敏模式
#[derive(Debug, Clone)]
pub struct MaskPattern {
    field_name: String,
    mask_type: MaskType,
}

impl MaskPattern {
    pub fn field(name: &str, mask_type: MaskType) -> Self {
        Self {
            field_name: name.to_lowercase(),
            mask_type,
        }
    }

    pub fn matches(&self, field: &str) -> bool {
        field.to_lowercase().contains(&self.field_name)
    }

    pub fn mask(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => serde_json::Value::String(self.mask_type.apply(s)),
            _ => value.clone(),
        }
    }
}

/// 脱敏类型
#[derive(Debug, Clone)]
pub enum MaskType {
    /// 完全脱敏
    Full,
    /// 部分脱敏（保留后 N 位）
    Partial { visible_chars: usize },
    /// 邮箱脱敏
    Email,
    /// 自定义
    Custom { pattern: String },
}

impl MaskType {
    pub fn apply(&self, value: &str) -> String {
        match self {
            Self::Full => "***".to_string(),
            Self::Partial { visible_chars } => {
                let len = value.len();
                if len <= *visible_chars {
                    "*".repeat(len)
                } else {
                    format!(
                        "{}{}",
                        "*".repeat(len - visible_chars),
                        &value[len - visible_chars..]
                    )
                }
            }
            Self::Email => {
                if let Some(at_pos) = value.find('@') {
                    let local = &value[..at_pos];
                    let domain = &value[at_pos..];
                    let masked_local = if local.len() > 2 {
                        format!("{}***{}", &local[..1], &local[local.len() - 1..])
                    } else {
                        "***".to_string()
                    };
                    format!("{}{}", masked_local, domain)
                } else {
                    "***".to_string()
                }
            }
            Self::Custom { pattern } => pattern.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_types() {
        assert_eq!(MaskType::Full.apply("secret"), "***");
        assert_eq!(
            MaskType::Partial { visible_chars: 4 }.apply("1234567890"),
            "******7890"
        );
        assert_eq!(
            MaskType::Email.apply("test@example.com"),
            "t***t@example.com"
        );
    }
}
