//! HSB 基础类型定义
//!
//! 定义系统中使用的核心类型标识符和枚举。

use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

// ============ ID 类型 ============

/// 系统标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SystemId(String);

impl SystemId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for SystemId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SystemId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// 机构标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganizationId(String);

impl OrganizationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrganizationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for OrganizationId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for OrganizationId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// 端点标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EndpointId(String);

impl EndpointId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EndpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EndpointId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for EndpointId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// 路由标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RouteId(String);

impl RouteId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 追踪标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

impl TraceId {
    pub fn new() -> Self {
        Self(Ulid::new().to_string())
    }

    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for TraceId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============ Topic 类型 ============

/// Topic 命名约定：`<domain>.<service>.<action>.<version>`
///
/// 示例：
/// - `medical.order.create.v1`
/// - `medical.image.upload.v1`
/// - `ai.infer.request.v1`
/// - `system.audit.log.v1`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Topic(String);

/// 知名 domain 常量
pub mod topic_domain {
    pub const MEDICAL: &str = "medical";
    pub const AI: &str = "ai";
    pub const SYSTEM: &str = "system";
    pub const INTEGRATION: &str = "integration";
}

impl Topic {
    /// 从已验证的 4 段构建 topic
    pub fn new(domain: &str, service: &str, action: &str, version: &str) -> Result<Self, String> {
        let parts = [domain, service, action, version];
        for p in &parts {
            if p.is_empty() {
                return Err("Topic 各段不能为空".to_string());
            }
            if p.contains('.') || p.contains(' ') {
                return Err(format!("Topic 段 '{}' 包含非法字符", p));
            }
        }
        if !version.starts_with('v') {
            return Err(format!("Topic 版本段应以 'v' 开头，实际为 '{}'", version));
        }
        Ok(Self(format!(
            "{}.{}.{}.{}",
            domain, service, action, version
        )))
    }

    /// 从完整字符串解析（会校验格式）
    pub fn parse(topic: impl Into<String>) -> Result<Self, String> {
        let s: String = topic.into();
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 4 {
            return Err(format!(
                "Topic 必须为 <domain>.<service>.<action>.<version> 格式 (4 段)，实际 {} 段: '{}'",
                parts.len(),
                s
            ));
        }
        Self::new(parts[0], parts[1], parts[2], parts[3])
    }

    /// 不做校验直接包装（用于反序列化等信任场景）
    pub fn from_string_unchecked(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 返回 domain 段
    pub fn domain(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }

    /// 返回 service 段
    pub fn service(&self) -> &str {
        self.0.splitn(3, '.').nth(1).unwrap_or("")
    }

    /// 返回 action 段
    pub fn action(&self) -> &str {
        self.0.splitn(4, '.').nth(2).unwrap_or("")
    }

    /// 返回 version 段
    pub fn version(&self) -> &str {
        self.0.rsplitn(2, '.').next().unwrap_or("")
    }

    /// 转为 NATS subject（原样返回，因为格式兼容）
    pub fn to_nats_subject(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Topic> for String {
    fn from(t: Topic) -> Self {
        t.0
    }
}

impl std::str::FromStr for Topic {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ============ 协议类型 ============

/// 支持的协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProtocolType {
    /// HTTP/REST
    Http,
    /// HL7 v2.x (MLLP)
    Hl7V2,
    /// HL7 v3 XML
    Hl7V3,
    /// HL7 FHIR R5
    FhirR5,
    /// DICOM
    Dicom,
    /// SOAP/WebService
    Soap,
    /// gRPC
    Grpc,
    /// TCP 原始协议
    TcpRaw,
    /// 消息队列 (RabbitMQ/Kafka)
    MessageQueue,
    /// 数据库连接端点
    Database,
    /// OpenAI 兼容大语言模型 API
    #[serde(rename = "OPENAI")]
    OpenAi,
    /// 私有协议
    Custom,
}

impl ProtocolType {
    /// 协议名称
    pub fn name(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Hl7V2 => "HL7V2",
            Self::Hl7V3 => "HL7V3",
            Self::FhirR5 => "FHIR_R5",
            Self::Dicom => "DICOM",
            Self::Soap => "SOAP",
            Self::Grpc => "gRPC",
            Self::TcpRaw => "TCP_RAW",
            Self::MessageQueue => "MQ",
            Self::Database => "DATABASE",
            Self::OpenAi => "OPENAI",
            Self::Custom => "CUSTOM",
        }
    }

    /// 默认端口
    pub fn default_port(&self) -> Option<u16> {
        match self {
            Self::Http => Some(80),
            Self::Hl7V2 => Some(2575), // MLLP 标准端口
            Self::Hl7V3 => None,
            Self::FhirR5 => Some(443),
            Self::Dicom => Some(104), // DICOM 标准端口
            Self::Soap => Some(80),
            Self::Grpc => Some(50051),
            Self::TcpRaw => None,
            Self::MessageQueue => None,
            Self::Database => None,
            Self::OpenAi => Some(443),
            Self::Custom => None,
        }
    }
}

impl fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "HTTP" => Ok(Self::Http),
            "HL7V2" | "HL7" | "MLLP" => Ok(Self::Hl7V2),
            "HL7V3" | "V3" | "CDA" => Ok(Self::Hl7V3),
            "FHIR" | "FHIR_R5" => Ok(Self::FhirR5),
            "DICOM" => Ok(Self::Dicom),
            "SOAP" => Ok(Self::Soap),
            "GRPC" => Ok(Self::Grpc),
            "TCP" | "TCP_RAW" => Ok(Self::TcpRaw),
            "MQ" | "MESSAGE_QUEUE" | "RABBITMQ" | "KAFKA" => Ok(Self::MessageQueue),
            "DATABASE" | "DB" | "POSTGRES" | "POSTGRESQL" | "ORACLE" | "MYSQL" | "SQLSERVER"
            | "MSSQL" | "HIVE" | "CLICKHOUSE" => Ok(Self::Database),
            "OPENAI" | "OPEN_AI" | "LLM" | "CHATGPT" => Ok(Self::OpenAi),
            "CUSTOM" | _ => Ok(Self::Custom),
        }
    }
}

// ============ 消息状态 ============

/// 消息处理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageStatus {
    /// 已接收，待处理
    Received,
    /// 处理中
    Processing,
    /// 路由中
    Routing,
    /// 转换中
    Transforming,
    /// 投递中
    Delivering,
    /// 已投递成功
    Delivered,
    /// 投递失败，待重试
    RetryPending,
    /// 重试中
    Retrying,
    /// 已进入死信队列
    DeadLetter,
    /// 处理成功
    Completed,
    /// 处理失败
    Failed,
    /// 已补偿
    Compensated,
}

impl MessageStatus {
    /// 是否为终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::DeadLetter | Self::Compensated
        )
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::RetryPending | Self::Retrying | Self::Failed)
    }
}

// ============ 消息优先级 ============

/// 消息优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessagePriority {
    /// 低优先级
    Low = 0,
    /// 普通优先级
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 紧急（急诊/危急值）
    Urgent = 3,
    /// 最高优先级（系统级）
    Critical = 4,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl MessagePriority {
    /// 返回小写字符串表示（用于 headers 等场景）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for MessagePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for MessagePriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "urgent" => Ok(Self::Urgent),
            "critical" => Ok(Self::Critical),
            _ => Err(format!("Unknown priority: {}", s)),
        }
    }
}

// ============ 医疗系统类型 ============

/// 医疗信息系统类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MedicalSystemType {
    /// 医院信息系统
    His,
    /// 实验室信息系统
    Lis,
    /// 放射信息系统
    Ris,
    /// 影像归档和通信系统
    Pacs,
    /// 电子病历系统
    Emr,
    /// 医院资源规划
    Hrp,
    /// 护理信息系统
    Nis,
    /// 手术麻醉系统
    Ors,
    /// 药房系统
    Pharmacy,
    /// 省级平台
    ProvincialPlatform,
    /// 医保系统
    MedicalInsurance,
    /// 第三方系统
    ThirdParty,
    /// 其他
    Other,
}

/// 机构类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrganizationType {
    GovernmentDepartment,
    Hospital,
    IndependentLegalEntity,
    Other,
}

impl OrganizationType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GovernmentDepartment => "GOVERNMENT_DEPARTMENT",
            Self::Hospital => "HOSPITAL",
            Self::IndependentLegalEntity => "INDEPENDENT_LEGAL_ENTITY",
            Self::Other => "OTHER",
        }
    }
}

impl std::str::FromStr for OrganizationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GOVERNMENT_DEPARTMENT" | "GOVERNMENT" => Ok(Self::GovernmentDepartment),
            "HOSPITAL" => Ok(Self::Hospital),
            "INDEPENDENT_LEGAL_ENTITY" | "LEGAL_ENTITY" => Ok(Self::IndependentLegalEntity),
            "OTHER" => Ok(Self::Other),
            other => Err(format!("Unknown organization type: {}", other)),
        }
    }
}

/// Endpoint 角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndpointRole {
    Producer,
    Consumer,
    Hybrid,
}

impl EndpointRole {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Producer => "PRODUCER",
            Self::Consumer => "CONSUMER",
            Self::Hybrid => "HYBRID",
        }
    }
}

impl std::str::FromStr for EndpointRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PRODUCER" => Ok(Self::Producer),
            "CONSUMER" => Ok(Self::Consumer),
            "HYBRID" | "BOTH" => Ok(Self::Hybrid),
            other => Err(format!("Unknown endpoint role: {}", other)),
        }
    }
}

/// Endpoint 连接加密算法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndpointEncryptionAlgorithm {
    None,
    Tls12,
    Tls13,
    MutualTls,
    Sm3,
}

impl EndpointEncryptionAlgorithm {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Tls12 => "TLS1_2",
            Self::Tls13 => "TLS1_3",
            Self::MutualTls => "MUTUAL_TLS",
            Self::Sm3 => "SM3",
        }
    }
}

impl std::str::FromStr for EndpointEncryptionAlgorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "NONE" => Ok(Self::None),
            "TLS1_2" | "TLS12" => Ok(Self::Tls12),
            "TLS1_3" | "TLS13" => Ok(Self::Tls13),
            "MUTUAL_TLS" | "MTLS" => Ok(Self::MutualTls),
            "SM3" => Ok(Self::Sm3),
            other => Err(format!("Unknown endpoint encryption algorithm: {}", other)),
        }
    }
}

impl MedicalSystemType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::His => "HIS",
            Self::Lis => "LIS",
            Self::Ris => "RIS",
            Self::Pacs => "PACS",
            Self::Emr => "EMR",
            Self::Hrp => "HRP",
            Self::Nis => "NIS",
            Self::Ors => "ORS",
            Self::Pharmacy => "PHARMACY",
            Self::ProvincialPlatform => "PROVINCIAL_PLATFORM",
            Self::MedicalInsurance => "MEDICAL_INSURANCE",
            Self::ThirdParty => "THIRD_PARTY",
            Self::Other => "OTHER",
        }
    }
}

impl std::str::FromStr for MedicalSystemType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "HIS" => Ok(Self::His),
            "LIS" => Ok(Self::Lis),
            "RIS" => Ok(Self::Ris),
            "PACS" => Ok(Self::Pacs),
            "EMR" => Ok(Self::Emr),
            "HRP" => Ok(Self::Hrp),
            "NIS" => Ok(Self::Nis),
            "ORS" => Ok(Self::Ors),
            "PHARMACY" => Ok(Self::Pharmacy),
            "PROVINCIAL_PLATFORM" => Ok(Self::ProvincialPlatform),
            "MEDICAL_INSURANCE" => Ok(Self::MedicalInsurance),
            "THIRD_PARTY" => Ok(Self::ThirdParty),
            "OTHER" => Ok(Self::Other),
            _ => Err(format!("Unknown medical system type: {}", s)),
        }
    }
}

// ============ HL7 消息类型 ============

/// HL7 v2 消息类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hl7MessageType {
    /// 消息类型（MSH-9.1）
    pub message_type: String,
    /// 触发事件（MSH-9.2）
    pub trigger_event: String,
    /// 消息结构（MSH-9.3）
    pub message_structure: Option<String>,
}

impl Hl7MessageType {
    pub fn new(message_type: &str, trigger_event: &str) -> Self {
        Self {
            message_type: message_type.to_string(),
            trigger_event: trigger_event.to_string(),
            message_structure: None,
        }
    }

    /// 常见消息类型
    pub fn adt_a01() -> Self {
        Self::new("ADT", "A01") // 入院
    }

    pub fn adt_a03() -> Self {
        Self::new("ADT", "A03") // 出院
    }

    pub fn orm_o01() -> Self {
        Self::new("ORM", "O01") // 医嘱
    }

    pub fn oru_r01() -> Self {
        Self::new("ORU", "R01") // 检验结果
    }

    pub fn siu_s12() -> Self {
        Self::new("SIU", "S12") // 预约
    }
}

// ============ 时间范围 ============

/// 时间范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

impl TimeRange {
    pub fn new(start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> chrono::Duration {
        self.end - self.start
    }

    pub fn contains(&self, time: chrono::DateTime<chrono::Utc>) -> bool {
        time >= self.start && time <= self.end
    }
}

// ============ 分页 ============

/// 分页请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRequest {
    pub page: u32,
    pub size: u32,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self { page: 1, size: 20 }
    }
}

impl PageRequest {
    pub fn new(page: u32, size: u32) -> Self {
        Self { page, size }
    }

    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.size
    }
}

/// 分页响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
    pub pages: u32,
}

impl<T> PageResponse<T> {
    pub fn new(items: Vec<T>, total: u64, request: &PageRequest) -> Self {
        let pages = ((total as f64) / (request.size as f64)).ceil() as u32;
        Self {
            items,
            total,
            page: request.page,
            size: request.size,
            pages,
        }
    }

    pub fn has_next(&self) -> bool {
        self.page < self.pages
    }

    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}
