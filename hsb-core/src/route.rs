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

    /// 目标端点列表（未配置分拣分支或没有分支命中时使用，即“默认出口”）
    pub targets: Vec<RouteTarget>,

    /// 分拣分支：按顺序判断，第一个命中的分支决定出口端点
    #[serde(default)]
    pub branches: Vec<RouteBranch>,

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
        if let Some(ref id) = self.system_id
            && id != "*"
            && id != msg.source_system.as_str()
        {
            return false;
        }

        // 检查协议
        if let Some(protocol) = self.protocol
            && protocol != msg.protocol
        {
            return false;
        }

        // 检查消息类型
        if let Some(ref pattern) = self.message_type_pattern {
            if let Some(ref msg_type) = msg.message_type {
                if let Ok(regex) = Regex::new(pattern)
                    && !regex.is_match(msg_type)
                {
                    return false;
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
    /// 消息基本属性：message_type / source_system / target_system / protocol / priority / correlation_id
    MessageField,
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

    /// 校验规则是否完整（返回面向用户的原因）
    pub fn validate(&self) -> Result<(), String> {
        if self.field_path.trim().is_empty() {
            return Err("条件必须选择/填写消息字段".to_string());
        }
        if matches!(
            self.operator,
            MatchOperator::Exists | MatchOperator::NotExists
        ) {
            return Ok(());
        }
        if self.value.trim().is_empty() {
            return Err(format!("字段 {} 的条件值不能为空", self.field_path));
        }
        if self.operator == MatchOperator::Regex && Regex::new(&self.value).is_err() {
            return Err(format!("字段 {} 的正则表达式无效", self.field_path));
        }
        Ok(())
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
            MatchSource::Header => msg.get_header(&self.field_path).cloned().or_else(|| {
                // HTTP 头名大小写不敏感
                msg.headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(&self.field_path))
                    .map(|(_, v)| v.clone())
            }),
            MatchSource::Payload => msg
                .get_payload_field(&self.field_path)
                .and_then(json_value_to_string),
            MatchSource::MessageField => match self.field_path.as_str() {
                "message_type" => msg.message_type.clone(),
                "source_system" => Some(msg.source_system.to_string()),
                "target_system" => msg.target_system.as_ref().map(|v| v.to_string()),
                "protocol" => Some(msg.protocol.name().to_string()),
                "priority" => Some(msg.priority.as_str().to_string()),
                "correlation_id" => msg.correlation_id.clone(),
                _ => None,
            },
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
                    "hospital_id" => msg.metadata.hospital_id.clone(),
                    "sending_application" => msg.metadata.sending_application.clone(),
                    "sending_facility" => msg.metadata.sending_facility.clone(),
                    "receiving_application" => msg.metadata.receiving_application.clone(),
                    "receiving_facility" => msg.metadata.receiving_facility.clone(),
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

/// 将 JSON 标量转为字符串（字符串原样，数字/布尔转文本；数组取第一个元素，便于 HL7 组件）
fn json_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(v) => Some(v.clone()),
        serde_json::Value::Number(v) => Some(v.to_string()),
        serde_json::Value::Bool(v) => Some(v.to_string()),
        serde_json::Value::Array(items) => items.first().and_then(json_value_to_string),
        _ => None,
    }
}

/// 条件组合逻辑
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionLogic {
    /// 全部条件满足
    #[default]
    And,
    /// 任一条件满足
    Or,
}

/// 条件组：多条字段条件按 AND / OR 组合
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConditionGroup {
    #[serde(default)]
    pub logic: ConditionLogic,
    #[serde(default)]
    pub rules: Vec<MatchRule>,
}

impl ConditionGroup {
    /// 空条件组视为恒真
    pub fn matches(&self, msg: &Message) -> bool {
        if self.rules.is_empty() {
            return true;
        }
        match self.logic {
            ConditionLogic::And => self.rules.iter().all(|rule| rule.matches(msg)),
            ConditionLogic::Or => self.rules.iter().any(|rule| rule.matches(msg)),
        }
    }
}

/// 分拣分支：条件命中后消息走该分支的出口端点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBranch {
    /// 分支名称
    pub name: String,

    /// 命中条件
    #[serde(default)]
    pub condition: ConditionGroup,

    /// 分支出口端点（首个为主出口，其余为备用）
    pub targets: Vec<RouteTarget>,
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

    /// 回复模式：出口端实时回复 / 延时回复（Webhook 回调入口端）
    #[serde(default)]
    pub reply_mode: ReplyMode,

    /// 延时回复时，入口端接收回调的 URL
    #[serde(default)]
    pub callback_url: Option<String>,

    /// 延时回复的等待时长（秒），超时后事务失效
    #[serde(default = "default_reply_timeout_secs")]
    pub reply_timeout_secs: u64,
}

/// 延时回复默认等待时长：24 小时
pub const DEFAULT_REPLY_TIMEOUT_SECS: u64 = 24 * 60 * 60;

fn default_reply_timeout_secs() -> u64 {
    DEFAULT_REPLY_TIMEOUT_SECS
}

/// 回复模式
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyMode {
    /// 出口端实时回复：出口端的同步响应直接回给入口端
    #[default]
    Realtime,
    /// 出口端延时回复：出口端稍后回复，总线通过 Webhook 回调入口端
    Deferred,
}

impl ReplyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Deferred => "deferred",
        }
    }
}

impl RouteOptions {
    /// 校验回复相关选项
    pub fn validate_reply(&self) -> HsbResult<()> {
        if self.reply_mode != ReplyMode::Deferred {
            return Ok(());
        }
        let url = self.callback_url.as_deref().map(str::trim).unwrap_or("");
        if url.is_empty() {
            return Err(HsbError::InvalidField {
                field: "callback_url".to_string(),
                reason: "延时回复必须填写入口端的回调 URL".to_string(),
            });
        }
        let has_host = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'));
        if !has_host || url.chars().any(char::is_whitespace) {
            return Err(HsbError::InvalidField {
                field: "callback_url".to_string(),
                reason: "回调 URL 必须是合法的 http:// 或 https:// 地址".to_string(),
            });
        }
        if self.reply_timeout_secs == 0 {
            return Err(HsbError::InvalidField {
                field: "reply_timeout_secs".to_string(),
                reason: "延时回复等待时长必须大于 0".to_string(),
            });
        }
        Ok(())
    }
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
            reply_mode: ReplyMode::Realtime,
            callback_url: None,
            reply_timeout_secs: DEFAULT_REPLY_TIMEOUT_SECS,
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

    /// 按分拣分支为消息选择出口：返回命中的分支名（默认出口为 None）和目标列表。
    ///
    /// 分支按顺序判断，首个命中的生效；都未命中则使用默认出口 `targets`（可能为空）。
    pub fn resolve_targets(&self, msg: &Message) -> (Option<&str>, &[RouteTarget]) {
        match self.branches.iter().find(|b| b.condition.matches(msg)) {
            Some(branch) => (Some(branch.name.as_str()), branch.targets.as_slice()),
            None => (None, self.targets.as_slice()),
        }
    }

    /// 校验分拣分支配置
    pub fn validate_branches(&self) -> HsbResult<()> {
        if self.targets.is_empty() && self.branches.is_empty() {
            return Err(HsbError::ValidationError {
                message: "At least one target is required".to_string(),
            });
        }
        for (index, branch) in self.branches.iter().enumerate() {
            let label = format!("branches[{}]", index + 1);
            if branch.targets.is_empty() {
                return Err(HsbError::InvalidField {
                    field: format!("{label}.targets"),
                    reason: format!("分拣分支“{}”至少需要一个出口端点", branch.name),
                });
            }
            if branch.condition.rules.is_empty() {
                return Err(HsbError::InvalidField {
                    field: format!("{label}.condition"),
                    reason: format!("分拣分支“{}”至少需要一个条件", branch.name),
                });
            }
            for rule in &branch.condition.rules {
                rule.validate().map_err(|reason| HsbError::InvalidField {
                    field: format!("{label}.condition"),
                    reason: format!("分拣分支“{}”：{}", branch.name, reason),
                })?;
            }
        }
        Ok(())
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
    branches: Vec<RouteBranch>,
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

    pub fn branch(mut self, branch: RouteBranch) -> Self {
        self.branches.push(branch);
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

        self.options.validate_reply()?;

        let route = Route {
            id,
            name,
            description: self.description,
            source_match: self.source_match.unwrap_or_else(SourceMatch::any),
            conditions: self.conditions,
            targets: self.targets,
            branches: self.branches,
            transformer_ids: self.transformer_ids,
            priority: self.priority,
            enabled: self.enabled,
            options: self.options,
        };
        route.validate_branches()?;
        Ok(route)
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

    fn rule(source: MatchSource, path: &str, op: MatchOperator, value: &str) -> MatchRule {
        MatchRule {
            name: path.to_string(),
            source,
            field_path: path.to_string(),
            operator: op,
            value: value.to_string(),
            negate: false,
        }
    }

    fn json_message(payload: serde_json::Value, message_type: &str) -> Message {
        MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .message_type(message_type)
            .payload(payload)
            .raw_payload("x")
            .build()
            .expect("Message should be valid")
    }

    fn branch(
        name: &str,
        logic: ConditionLogic,
        rules: Vec<MatchRule>,
        target: &str,
    ) -> RouteBranch {
        RouteBranch {
            name: name.to_string(),
            condition: ConditionGroup { logic, rules },
            targets: vec![RouteTarget::primary(target)],
        }
    }

    #[test]
    fn branches_pick_first_match_or_default() {
        let route = RouteBuilder::new()
            .id("sorter")
            .name("sorter")
            .branch(branch(
                "检验",
                ConditionLogic::And,
                vec![
                    rule(
                        MatchSource::MessageField,
                        "message_type",
                        MatchOperator::Equals,
                        "ORDER",
                    ),
                    rule(
                        MatchSource::Payload,
                        "order.kind",
                        MatchOperator::Equals,
                        "LAB",
                    ),
                ],
                "LIS",
            ))
            .branch(branch(
                "影像或病理",
                ConditionLogic::Or,
                vec![
                    rule(
                        MatchSource::Payload,
                        "order.kind",
                        MatchOperator::Equals,
                        "RIS",
                    ),
                    rule(
                        MatchSource::Payload,
                        "order.priority",
                        MatchOperator::GreaterThan,
                        "5",
                    ),
                ],
                "RIS",
            ))
            .target(RouteTarget::primary("DEFAULT"))
            .build()
            .expect("route should build");

        let pick = |msg: &Message| {
            let (name, targets) = route.resolve_targets(msg);
            (name.map(str::to_string), targets[0].endpoint_id.to_string())
        };

        // AND：两个条件都满足
        let lab = json_message(serde_json::json!({"order": {"kind": "LAB"}}), "ORDER");
        assert_eq!(pick(&lab), (Some("检验".to_string()), "LIS".to_string()));

        // AND：只满足其一，落入后面的分支/默认
        let lab_wrong_type = json_message(serde_json::json!({"order": {"kind": "LAB"}}), "RESULT");
        assert_eq!(pick(&lab_wrong_type), (None, "DEFAULT".to_string()));

        // OR：数字字段也可比较
        let urgent = json_message(
            serde_json::json!({"order": {"kind": "X", "priority": 9}}),
            "ORDER",
        );
        assert_eq!(
            pick(&urgent),
            (Some("影像或病理".to_string()), "RIS".to_string())
        );

        // 都不命中 -> 默认出口
        let other = json_message(
            serde_json::json!({"order": {"kind": "X", "priority": 1}}),
            "ORDER",
        );
        assert_eq!(pick(&other), (None, "DEFAULT".to_string()));
    }

    #[test]
    fn payload_path_supports_array_index_and_scalar_types() {
        let msg = json_message(
            serde_json::json!({"PID": {"PID_5": ["Doe", "John"], "flag": true, "age": 42}}),
            "ADT",
        );
        let r = |path: &str, value: &str| {
            rule(MatchSource::Payload, path, MatchOperator::Equals, value)
        };
        assert!(r("PID.PID_5.1", "John").matches(&msg));
        assert!(r("PID.PID_5", "Doe").matches(&msg));
        assert!(r("PID.flag", "true").matches(&msg));
        assert!(r("PID.age", "42").matches(&msg));
        assert!(!r("PID.missing", "x").matches(&msg));
    }

    #[test]
    fn header_lookup_is_case_insensitive() {
        let mut msg = json_message(serde_json::json!({}), "ADT");
        msg.headers.insert("x-dept".to_string(), "ICU".to_string());
        assert!(rule(MatchSource::Header, "X-Dept", MatchOperator::Equals, "ICU").matches(&msg));
    }

    #[test]
    fn branch_validation_rejects_incomplete_configuration() {
        let build = |rules: Vec<MatchRule>, targets: Vec<RouteTarget>| {
            RouteBuilder::new()
                .id("r")
                .name("r")
                .branch(RouteBranch {
                    name: "b".to_string(),
                    condition: ConditionGroup {
                        logic: ConditionLogic::And,
                        rules,
                    },
                    targets,
                })
                .build()
        };
        let ok_rule = rule(MatchSource::Payload, "a", MatchOperator::Equals, "1");
        assert!(build(vec![ok_rule.clone()], vec![RouteTarget::primary("X")]).is_ok());
        assert!(build(vec![], vec![RouteTarget::primary("X")]).is_err());
        assert!(build(vec![ok_rule.clone()], vec![]).is_err());
        assert!(
            build(
                vec![rule(MatchSource::Payload, "a", MatchOperator::Equals, "")],
                vec![RouteTarget::primary("X")]
            )
            .is_err()
        );
        assert!(
            build(
                vec![rule(MatchSource::Payload, "a", MatchOperator::Regex, "([")],
                vec![RouteTarget::primary("X")]
            )
            .is_err()
        );
        assert!(
            build(
                vec![rule(MatchSource::Payload, "a", MatchOperator::Exists, "")],
                vec![RouteTarget::primary("X")]
            )
            .is_ok()
        );
        // 既无分支也无默认出口
        assert!(RouteBuilder::new().id("r").name("r").build().is_err());
    }
}
