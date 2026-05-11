//! HSB 常量定义

/// HL7 相关常量
pub mod hl7 {
    /// MLLP 起始字节
    pub const MLLP_START_BLOCK: u8 = 0x0B;
    /// MLLP 结束字节
    pub const MLLP_END_BLOCK: u8 = 0x1C;
    /// MLLP 回车符
    pub const MLLP_CARRIAGE_RETURN: u8 = 0x0D;
    /// HL7 段分隔符
    pub const SEGMENT_SEPARATOR: char = '\r';
    /// HL7 字段分隔符
    pub const FIELD_SEPARATOR: char = '|';
    /// HL7 组件分隔符
    pub const COMPONENT_SEPARATOR: char = '^';
    /// HL7 重复分隔符
    pub const REPETITION_SEPARATOR: char = '~';
    /// HL7 子组件分隔符
    pub const SUBCOMPONENT_SEPARATOR: char = '&';
    /// HL7 转义字符
    pub const ESCAPE_CHARACTER: char = '\\';
    /// 默认 MLLP 端口
    pub const DEFAULT_MLLP_PORT: u16 = 2575;
}

/// DICOM 相关常量
pub mod dicom {
    /// DICOM 默认端口
    pub const DEFAULT_PORT: u16 = 104;
    /// DICOM TLS 端口
    pub const DEFAULT_TLS_PORT: u16 = 2762;
    /// 最大 PDU 长度
    pub const MAX_PDU_LENGTH: u32 = 16384;
}

/// FHIR 相关常量
pub mod fhir {
    /// FHIR 版本
    pub const FHIR_VERSION: &str = "5.0.0";
    /// FHIR MIME 类型
    pub const CONTENT_TYPE_JSON: &str = "application/fhir+json";
    pub const CONTENT_TYPE_XML: &str = "application/fhir+xml";
}

/// HTTP 相关常量
pub mod http {
    /// 自定义 Header：追踪 ID
    pub const HEADER_TRACE_ID: &str = "X-HSB-Trace-Id";
    /// 自定义 Header：源系统
    pub const HEADER_SOURCE_SYSTEM: &str = "X-HSB-Source-System";
    /// 自定义 Header：目标系统
    pub const HEADER_TARGET_SYSTEM: &str = "X-HSB-Target-System";
    /// 自定义 Header：消息类型
    pub const HEADER_MESSAGE_TYPE: &str = "X-HSB-Message-Type";
    /// 自定义 Header：优先级
    pub const HEADER_PRIORITY: &str = "X-HSB-Priority";
    /// 自定义 Header：关联 ID
    pub const HEADER_CORRELATION_ID: &str = "X-HSB-Correlation-Id";
}

/// 消息队列常量（兼容 topic 格式）
pub mod mq {
    /// 默认交换机
    pub const DEFAULT_EXCHANGE: &str = "hsb.exchange";
    /// 路由队列
    pub const ROUTE_QUEUE: &str = "system.route.dispatch.v1";
    /// 重试队列
    pub const RETRY_QUEUE: &str = "system.retry.pending.v1";
    /// 死信队列
    pub const DLQ_QUEUE: &str = "system.dlq.entry.v1";
    /// 审计队列
    pub const AUDIT_QUEUE: &str = "system.audit.log.v1";
}

/// 缓存表名 & 键前缀（PostgreSQL UNLOGGED 表）
pub mod cache {
    /// 缓存表名
    pub const TABLE_NAME: &str = "hsb_cache";
    /// 路由缓存
    pub const ROUTE_PREFIX: &str = "route:";
    /// 端点缓存
    pub const ENDPOINT_PREFIX: &str = "endpoint:";
    /// 会话缓存
    pub const SESSION_PREFIX: &str = "session:";
    /// 限流缓存
    pub const RATE_LIMIT_PREFIX: &str = "ratelimit:";
}

/// 系统限制
pub mod limits {
    /// 最大消息大小（10MB）
    pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;
    /// 最大 Header 数量
    pub const MAX_HEADERS: usize = 100;
    /// 最大路由深度
    pub const MAX_ROUTE_DEPTH: usize = 10;
    /// 最大重试次数
    pub const MAX_RETRY_ATTEMPTS: u32 = 10;
    /// 默认超时（秒）
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
    /// 最大并发连接数
    pub const MAX_CONCURRENT_CONNECTIONS: usize = 10000;
}

/// 预定义 Topic 常量
///
/// 格式：`<domain>.<service>.<action>.<version>`
pub mod topics {
    // ---- medical 领域 ----
    pub const MEDICAL_ORDER_CREATE: &str = "medical.order.create.v1";
    pub const MEDICAL_ORDER_UPDATE: &str = "medical.order.update.v1";
    pub const MEDICAL_ORDER_CANCEL: &str = "medical.order.cancel.v1";
    pub const MEDICAL_PATIENT_ADMIT: &str = "medical.patient.admit.v1";
    pub const MEDICAL_PATIENT_DISCHARGE: &str = "medical.patient.discharge.v1";
    pub const MEDICAL_PATIENT_TRANSFER: &str = "medical.patient.transfer.v1";
    pub const MEDICAL_RESULT_REPORT: &str = "medical.result.report.v1";
    pub const MEDICAL_IMAGE_UPLOAD: &str = "medical.image.upload.v1";
    pub const MEDICAL_IMAGE_QUERY: &str = "medical.image.query.v1";
    pub const MEDICAL_SCHEDULE_CREATE: &str = "medical.schedule.create.v1";

    // ---- ai 领域 ----
    pub const AI_INFER_REQUEST: &str = "ai.infer.request.v1";
    pub const AI_INFER_RESPONSE: &str = "ai.infer.response.v1";

    // ---- integration 领域 ----
    pub const INTEGRATION_SYNC_REQUEST: &str = "integration.sync.request.v1";
    pub const INTEGRATION_SYNC_RESPONSE: &str = "integration.sync.response.v1";

    // ---- system 领域 ----
    pub const SYSTEM_AUDIT_LOG: &str = "system.audit.log.v1";
    pub const SYSTEM_CLUSTER_HEARTBEAT: &str = "system.cluster.heartbeat.v1";
    pub const SYSTEM_CLUSTER_SYNC: &str = "system.cluster.sync.v1";
    pub const SYSTEM_DLQ_ENTRY: &str = "system.dlq.entry.v1";
    pub const SYSTEM_HEALTH_CHECK: &str = "system.health.check.v1";
}
