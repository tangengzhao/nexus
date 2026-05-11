//! HSB 统一错误定义
//!
//! 所有 HSB 模块共用的错误类型，遵循 Rust 错误处理最佳实践。

use thiserror::Error;
use ulid::Ulid;

/// HSB 统一结果类型
pub type HsbResult<T> = Result<T, HsbError>;

/// HSB 错误类型
#[derive(Error, Debug)]
pub enum HsbError {
    // ============ 配置错误 ============
    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Missing configuration: {key}")]
    MissingConfig { key: String },

    // ============ 协议错误 ============
    #[error("Protocol error [{protocol}]: {message}")]
    ProtocolError { protocol: String, message: String },

    #[error("Protocol not supported: {protocol}")]
    ProtocolNotSupported { protocol: String },

    #[error("Message parse error: {message}")]
    ParseError { message: String },

    #[error("Message serialization error: {message}")]
    SerializationError { message: String },

    // ============ 路由错误 ============
    #[error("Route not found for message: {message_id}")]
    RouteNotFound { message_id: Ulid },

    #[error("Route condition error: {message}")]
    RouteConditionError { message: String },

    #[error("No target endpoint matched for route: {route_id}")]
    NoTargetMatched { route_id: String },

    // ============ 转换错误 ============
    #[error("Transformation error: {message}")]
    TransformError { message: String },

    #[error("Field mapping error: source={source_field}, target={target_field}, reason={reason}")]
    FieldMappingError {
        source_field: String,
        target_field: String,
        reason: String,
    },

    // ============ 传输错误 ============
    #[error("Connection error to {endpoint}: {message}")]
    ConnectionError { endpoint: String, message: String },

    #[error("Timeout error: operation={operation}, timeout_ms={timeout_ms}")]
    TimeoutError { operation: String, timeout_ms: u64 },

    #[error("Transport error: {message}")]
    TransportError { message: String },

    // ============ 消息队列错误 ============
    #[error("Queue error: {message}")]
    QueueError { message: String },

    #[error("Message delivery failed after {attempts} attempts: {reason}")]
    DeliveryFailed { attempts: u32, reason: String },

    #[error("Dead letter queue error: {message}")]
    DlqError { message: String },

    // ============ 数据库错误 ============
    #[error("Database error: {message}")]
    DatabaseError { message: String },

    #[error("Record not found: {entity}[{id}]")]
    NotFound { entity: String, id: String },

    #[error("Duplicate record: {entity}[{id}]")]
    DuplicateRecord { entity: String, id: String },

    // ============ 认证授权错误 ============
    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },

    #[error("Authorization denied: {message}")]
    AuthorizationError { message: String },

    #[error("Token expired")]
    TokenExpired,

    #[error("Invalid token: {message}")]
    InvalidToken { message: String },

    // ============ 验证错误 ============
    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Invalid field [{field}]: {reason}")]
    InvalidField { field: String, reason: String },

    // ============ 工作流错误 ============
    #[error("Workflow error: {workflow_id}, step={step}, reason={reason}")]
    WorkflowError {
        workflow_id: String,
        step: String,
        reason: String,
    },

    #[error("Compensation failed: {workflow_id}, reason={reason}")]
    CompensationFailed { workflow_id: String, reason: String },

    // ============ 系统错误 ============
    #[error("Internal error: {message}")]
    InternalError { message: String },

    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },

    #[error("Service unavailable: {service}")]
    ServiceUnavailable { service: String },

    // ============ 外部错误包装 ============
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("ULID error: {0}")]
    UlidError(String),
}

impl HsbError {
    /// 获取错误码（用于 API 响应）
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::ConfigError { .. } => "CONFIG_ERROR",
            Self::MissingConfig { .. } => "MISSING_CONFIG",
            Self::ProtocolError { .. } => "PROTOCOL_ERROR",
            Self::ProtocolNotSupported { .. } => "PROTOCOL_NOT_SUPPORTED",
            Self::ParseError { .. } => "PARSE_ERROR",
            Self::SerializationError { .. } => "SERIALIZATION_ERROR",
            Self::RouteNotFound { .. } => "ROUTE_NOT_FOUND",
            Self::RouteConditionError { .. } => "ROUTE_CONDITION_ERROR",
            Self::NoTargetMatched { .. } => "NO_TARGET_MATCHED",
            Self::TransformError { .. } => "TRANSFORM_ERROR",
            Self::FieldMappingError { .. } => "FIELD_MAPPING_ERROR",
            Self::ConnectionError { .. } => "CONNECTION_ERROR",
            Self::TimeoutError { .. } => "TIMEOUT_ERROR",
            Self::TransportError { .. } => "TRANSPORT_ERROR",
            Self::QueueError { .. } => "QUEUE_ERROR",
            Self::DeliveryFailed { .. } => "DELIVERY_FAILED",
            Self::DlqError { .. } => "DLQ_ERROR",
            Self::DatabaseError { .. } => "DATABASE_ERROR",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::DuplicateRecord { .. } => "DUPLICATE_RECORD",
            Self::AuthenticationError { .. } => "AUTHENTICATION_ERROR",
            Self::AuthorizationError { .. } => "AUTHORIZATION_ERROR",
            Self::TokenExpired => "TOKEN_EXPIRED",
            Self::InvalidToken { .. } => "INVALID_TOKEN",
            Self::ValidationError { .. } => "VALIDATION_ERROR",
            Self::InvalidField { .. } => "INVALID_FIELD",
            Self::WorkflowError { .. } => "WORKFLOW_ERROR",
            Self::CompensationFailed { .. } => "COMPENSATION_FAILED",
            Self::InternalError { .. } => "INTERNAL_ERROR",
            Self::ResourceExhausted { .. } => "RESOURCE_EXHAUSTED",
            Self::ServiceUnavailable { .. } => "SERVICE_UNAVAILABLE",
            Self::IoError(_) => "IO_ERROR",
            Self::JsonError(_) => "JSON_ERROR",
            Self::UlidError(_) => "ULID_ERROR",
        }
    }

    /// 是否为可重试错误
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ConnectionError { .. }
                | Self::TimeoutError { .. }
                | Self::TransportError { .. }
                | Self::ServiceUnavailable { .. }
                | Self::ResourceExhausted { .. }
        )
    }

    /// HTTP 状态码映射
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::AuthenticationError { .. } | Self::InvalidToken { .. } | Self::TokenExpired => {
                401
            }
            Self::AuthorizationError { .. } => 403,
            Self::NotFound { .. } | Self::RouteNotFound { .. } => 404,
            Self::DuplicateRecord { .. } => 409,
            Self::ValidationError { .. } | Self::InvalidField { .. } | Self::ParseError { .. } => {
                400
            }
            Self::TimeoutError { .. } => 408,
            Self::ResourceExhausted { .. } => 429,
            Self::ServiceUnavailable { .. } => 503,
            _ => 500,
        }
    }
}

/// 错误上下文扩展 trait
pub trait ErrorContext<T> {
    /// 添加上下文信息
    fn context(self, message: impl Into<String>) -> HsbResult<T>;

    /// 添加字段上下文
    fn with_field(self, field: &str) -> HsbResult<T>;
}

impl<T, E: std::error::Error> ErrorContext<T> for Result<T, E> {
    fn context(self, message: impl Into<String>) -> HsbResult<T> {
        self.map_err(|e| HsbError::InternalError {
            message: format!("{}: {}", message.into(), e),
        })
    }

    fn with_field(self, field: &str) -> HsbResult<T> {
        self.map_err(|e| HsbError::InvalidField {
            field: field.to_string(),
            reason: e.to_string(),
        })
    }
}
