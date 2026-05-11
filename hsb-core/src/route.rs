//! 路由规则定义
//!
//! 定义消息路由的匹配规则和目标选择。

use crate::message::Message;
use hsb_common::{HsbError, HsbResult, ProtocolType, RouteId, SystemId};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// 路由规则
///
/// 定义从源端点到目标端点的消息路由规则。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// 路由唯一标识
    pub id: RouteId,

    /// 路由名称
    pub name: String,

    /// 路由描述
    pub description: Option<String>,

    /// 源端点匹配
    pub source_match: SourceMatch,

    /// 匹配条件列表
    pub conditions: Vec<MatchRule>,

    /// 目标端点列表
    pub targets: Vec<RouteTarget>,

    /// 转换器 ID 列表
    pub transformer_ids: Vec<String>,

    /// 路由优先级（数字越大优先级越高）
    pub priority: i32,

    /// 是否启用
    pub enabled: bool,

    /// 路由选项
    pub options: RouteOptions,
}

/// 源端点匹配
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceMatch {
    /// 源系统 ID（精确匹配或通配符）
    pub system_id: Option<String>,

    /// 协议类型
    pub protocol: Option<ProtocolType>,

    /// 消息类型模式（正则）
    pub message_type_pattern: Option<String>,
}

impl SourceMatch {
    /// 匹配所有源
    pub fn any() -> Self {
        Self {
            system_id: None,
            protocol: None,
            message_type_pattern: None,
        }
    }

    /// 匹配指定系统
    pub fn system(id: impl Into<String>) -> Self {
        Self {
            system_id: Some(id.into()),
            protocol: None,
            message_type_pattern: None,
        }
    }

    /// 检查是否匹配消息
    pub fn matches(&self, msg: &Message) -> bool {
        // 检查系统 ID
        if let Some(ref id) = self.system_id {
            if id != "*" && id != msg.source_system.as_str() {
                return false;
            }
        }

        // 检查协议
        if let Some(protocol) = self.protocol {
            if protocol != msg.protocol {
                return false;
            }
        }

        // 检查消息类型
        if let Some(ref pattern) = self.message_type_pattern {
            if let Some(ref msg_type) = msg.message_type {
                if let Ok(regex) = Regex::new(pattern) {
                    if !regex.is_match(msg_type) {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// 匹配规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRule {
    /// 规则名称
    pub name: String,

    /// 匹配字段来源
    pub source: MatchSource,

    /// 字段路径（如 header.Content-Type 或 payload.patient.id）
    pub field_path: String,

    /// 匹配操作符
    pub operator: MatchOperator,

    /// 匹配值
    pub value: String,

    /// 是否取反
    pub negate: bool,
}

/// 匹配字段来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchSource {
    /// 消息头部
    Header,
    /// 消息体（JSON）
    Payload,
    /// 原始报文
    RawPayload,
    /// 元数据
    Metadata,
}

/// 匹配操作符
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOperator {
    /// 等于
    Equals,
    /// 不等于
    NotEquals,
    /// 包含
    Contains,
    /// 以...开头
    StartsWith,
    /// 以...结尾
    EndsWith,
    /// 正则匹配
    Regex,
    /// 存在（非空）
    Exists,
    /// 不存在（空）
    NotExists,
    /// 在列表中
    In,
    /// 不在列表中
    NotIn,
    /// 大于
    GreaterThan,
    /// 小于
    LessThan,
}

impl MatchRule {
    /// 创建等于规则
    pub fn equals(
        name: impl Into<String>,
        source: MatchSource,
        field_path: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source,
            field_path: field_path.into(),
            operator: MatchOperator::Equals,
            value: value.into(),
            negate: false,
        }
    }

    /// 创建正则匹配规则
    pub fn regex(
        name: impl Into<String>,
        source: MatchSource,
        field_path: impl Into<String>,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source,
            field_path: field_path.into(),
            operator: MatchOperator::Regex,
            value: pattern.into(),
            negate: false,
        }
    }

    /// 检查消息是否匹配此规则
    pub fn matches(&self, msg: &Message) -> bool {
        let field_value = self.extract_field_value(msg);
        let result = self.check_operator(&field_value);
        if self.negate { !result } else { result }
    }

    /// 从消息中提取字段值
    fn extract_field_value(&self, msg: &Message) -> Option<String> {
        match self.source {
            MatchSource::Header => msg.get_header(&self.field_path).cloned(),
            MatchSource::Payload => msg
                .get_payload_field(&self.field_path)
                .and_then(|v| v.as_str().map(String::from)),
            MatchSource::RawPayload => {
                // 从原始报文中提取（简单实现：整个报文作为字符串）
                String::from_utf8(msg.raw_payload.to_vec()).ok()
            }
            MatchSource::Metadata => {
                // 从元数据中提取
                match self.field_path.as_str() {
                    "patient_id" => msg.metadata.patient_id.clone(),
                    "visit_id" => msg.metadata.visit_id.clone(),
                    "order_id" => msg.metadata.order_id.clone(),
                    "department_code" => msg.metadata.department_code.clone(),
                    _ => None,
                }
            }
        }
    }

    /// 检查操作符匹配
    fn check_operator(&self, field_value: &Option<String>) -> bool {
        match &self.operator {
            MatchOperator::Exists => field_value.is_some(),
            MatchOperator::NotExists => field_value.is_none(),
            _ => {
                if let Some(value) = field_value {
                    match &self.operator {
                        MatchOperator::Equals => value == &self.value,
                        MatchOperator::NotEquals => value != &self.value,
                        MatchOperator::Contains => value.contains(&self.value),
                        MatchOperator::StartsWith => value.starts_with(&self.value),
                        MatchOperator::EndsWith => value.ends_with(&self.value),
                        MatchOperator::Regex => Regex::new(&self.value)
                            .map(|r| r.is_match(value))
                            .unwrap_or(false),
                        MatchOperator::In => self.value.split(',').any(|v| v.trim() == value),
                        MatchOperator::NotIn => !self.value.split(',').any(|v| v.trim() == value),
                        MatchOperator::GreaterThan => {
                            if let (Ok(a), Ok(b)) =
                                (value.parse::<f64>(), self.value.parse::<f64>())
                            {
                                a > b
                            } else {
                                value > &self.value
                            }
                        }
                        MatchOperator::LessThan => {
                            if let (Ok(a), Ok(b)) =
                                (value.parse::<f64>(), self.value.parse::<f64>())
                            {
                                a < b
                            } else {
                                value < &self.value
                            }
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}

/// 路由目标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTarget {
    /// 目标端点 ID
    pub endpoint_id: SystemId,

    /// 目标权重（用于负载均衡）
    pub weight: u32,

    /// 是否为主要目标
    pub primary: bool,

    /// 是否为备用目标
    pub fallback: bool,

    /// 目标特定的转换器 ID
    pub transformer_ids: Vec<String>,
}

impl RouteTarget {
    /// 创建主要目标
    pub fn primary(endpoint_id: impl Into<SystemId>) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            weight: 100,
            primary: true,
            fallback: false,
            transformer_ids: Vec::new(),
        }
    }

    /// 创建备用目标
    pub fn fallback(endpoint_id: impl Into<SystemId>) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            weight: 0,
            primary: false,
            fallback: true,
            transformer_ids: Vec::new(),
        }
    }

    /// 创建带权重的目标
    pub fn weighted(endpoint_id: impl Into<SystemId>, weight: u32) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            weight,
            primary: false,
            fallback: false,
            transformer_ids: Vec::new(),
        }
    }
}

/// 路由选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteOptions {
    /// 投递策略
    pub delivery_mode: DeliveryMode,

    /// 超时（毫秒）
    pub timeout_ms: u64,

    /// 是否异步投递
    pub async_delivery: bool,

    /// 是否需要确认
    pub require_ack: bool,

    /// 是否记录审计日志
    pub audit_enabled: bool,

    /// 失败时是否进入 DLQ
    pub dlq_on_failure: bool,
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            delivery_mode: DeliveryMode::AtLeastOnce,
            timeout_ms: 30000,
            async_delivery: false,
            require_ack: true,
            audit_enabled: true,
            dlq_on_failure: true,
        }
    }
}

/// 投递模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    /// 最多一次
    AtMostOnce,
    /// 至少一次
    AtLeastOnce,
    /// 恰好一次
    ExactlyOnce,
}

impl Route {
    /// 检查消息是否匹配此路由
    pub fn matches(&self, msg: &Message) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查源匹配
        if !self.source_match.matches(msg) {
            return false;
        }

        // 检查所有条件
        self.conditions.iter().all(|rule| rule.matches(msg))
    }

    /// 获取主要目标
    pub fn primary_target(&self) -> Option<&RouteTarget> {
        self.targets.iter().find(|t| t.primary)
    }

    /// 获取备用目标
    pub fn fallback_targets(&self) -> Vec<&RouteTarget> {
        self.targets.iter().filter(|t| t.fallback).collect()
    }

    /// 获取所有活跃目标
    pub fn active_targets(&self) -> Vec<&RouteTarget> {
        self.targets.iter().filter(|t| !t.fallback).collect()
    }
}

/// 路由构建器
#[derive(Debug, Default)]
pub struct RouteBuilder {
    id: Option<RouteId>,
    name: Option<String>,
    description: Option<String>,
    source_match: Option<SourceMatch>,
    conditions: Vec<MatchRule>,
    targets: Vec<RouteTarget>,
    transformer_ids: Vec<String>,
    priority: i32,
    enabled: bool,
    options: RouteOptions,
}

impl RouteBuilder {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(RouteId::new(id));
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn source(mut self, source_match: SourceMatch) -> Self {
        self.source_match = Some(source_match);
        self
    }

    pub fn condition(mut self, rule: MatchRule) -> Self {
        self.conditions.push(rule);
        self
    }

    pub fn target(mut self, target: RouteTarget) -> Self {
        self.targets.push(target);
        self
    }

    pub fn transformer(mut self, id: impl Into<String>) -> Self {
        self.transformer_ids.push(id.into());
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn options(mut self, options: RouteOptions) -> Self {
        self.options = options;
        self
    }

    pub fn build(self) -> HsbResult<Route> {
        let id = self.id.ok_or_else(|| HsbError::ValidationError {
            message: "Route id is required".to_string(),
        })?;

        let name = self.name.ok_or_else(|| HsbError::ValidationError {
            message: "Route name is required".to_string(),
        })?;

        if self.targets.is_empty() {
            return Err(HsbError::ValidationError {
                message: "At least one target is required".to_string(),
            });
        }

        Ok(Route {
            id,
            name,
            description: self.description,
            source_match: self.source_match.unwrap_or_else(SourceMatch::any),
            conditions: self.conditions,
            targets: self.targets,
            transformer_ids: self.transformer_ids,
            priority: self.priority,
            enabled: self.enabled,
            options: self.options,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageBuilder;
    use hsb_common::ProtocolType;

    #[test]
    fn test_route_matching() {
        let route = RouteBuilder::new()
            .id("route_1")
            .name("HIS to LIS")
            .source(SourceMatch::system("HIS"))
            .condition(MatchRule::equals(
                "msg_type",
                MatchSource::Metadata,
                "department_code",
                "LAB",
            ))
            .target(RouteTarget::primary("LIS"))
            .build()
            .expect("Route should be valid");

        let mut msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Hl7V2)
            .raw_payload("test")
            .build()
            .expect("Message should be valid");

        msg.metadata.department_code = Some("LAB".to_string());

        assert!(route.matches(&msg));
    }
}
