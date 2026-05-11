//! 端点（Endpoint）定义
//!
//! 端点代表一个医疗系统的连接配置。

use chrono::{DateTime, Utc};
use hsb_common::{
    EndpointEncryptionAlgorithm, EndpointId, EndpointRole, MedicalSystemType, OrganizationId,
    ProtocolType, SystemId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// 端点生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EndpointLifecycleStatus {
    Draft,
    Active,
    Disabled,
    Deprecated,
    Retired,
}

impl Default for EndpointLifecycleStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl std::str::FromStr for EndpointLifecycleStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DRAFT" => Ok(Self::Draft),
            "ACTIVE" => Ok(Self::Active),
            "DISABLED" => Ok(Self::Disabled),
            "DEPRECATED" => Ok(Self::Deprecated),
            "RETIRED" => Ok(Self::Retired),
            _ => Err(format!("Unknown endpoint lifecycle status: {}", s)),
        }
    }
}

/// 端点安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSecurity {
    /// 外部密钥管理系统中的密钥引用
    pub secret_ref: Option<String>,
    /// 是否强制 TLS
    pub require_tls: bool,
    /// 是否允许跳过证书校验
    pub allow_insecure_skip_verify: bool,
    /// 白名单 IP 段
    pub allowed_ip_ranges: Vec<String>,
    /// 是否在日志中脱敏凭证
    pub mask_credentials_in_logs: bool,
    /// 连接/完整性算法选择
    pub encryption_algorithm: EndpointEncryptionAlgorithm,
    /// 凭证过期时间
    pub credential_expires_at: Option<DateTime<Utc>>,
    /// 最近一次轮换时间
    pub credential_last_rotated_at: Option<DateTime<Utc>>,
}

impl Default for EndpointSecurity {
    fn default() -> Self {
        Self {
            secret_ref: None,
            require_tls: false,
            allow_insecure_skip_verify: false,
            allowed_ip_ranges: Vec::new(),
            mask_credentials_in_logs: true,
            encryption_algorithm: EndpointEncryptionAlgorithm::None,
            credential_expires_at: None,
            credential_last_rotated_at: None,
        }
    }
}

/// 端点运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointRuntimeStatus {
    pub endpoint_id: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub circuit_state: Option<String>,
    pub consecutive_failures: u32,
    pub last_check_at: Option<DateTime<Utc>>,
    pub last_delivery_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl EndpointRuntimeStatus {
    pub fn new(endpoint_id: impl Into<String>) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            healthy: false,
            latency_ms: None,
            last_error: None,
            circuit_state: Some("closed".to_string()),
            consecutive_failures: 0,
            last_check_at: None,
            last_delivery_at: None,
            updated_at: Utc::now(),
        }
    }
}

/// 端点版本快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointVersionRecord {
    pub endpoint_id: String,
    pub version: u32,
    pub snapshot: Endpoint,
    pub changed_at: DateTime<Utc>,
    pub changed_by: Option<String>,
    pub change_note: Option<String>,
}

/// 系统端点
///
/// 代表一个 HIS / LIS / PACS 等医疗系统的连接信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// 端点唯一标识
    pub id: EndpointId,

    /// 所属机构
    pub organization_id: OrganizationId,

    /// 所属系统
    pub system_id: SystemId,

    /// 端点角色
    pub roles: Vec<EndpointRole>,

    /// 端点名称
    pub name: String,

    /// 端点描述
    pub description: Option<String>,

    /// 系统类型
    pub system_type: MedicalSystemType,

    /// 协议类型
    pub protocol: ProtocolType,

    /// 连接配置
    pub connection: ConnectionConfig,

    /// 认证配置
    pub auth: Option<AuthConfig>,

    /// 端点配置
    pub config: EndpointConfig,

    /// 是否启用
    pub enabled: bool,

    /// 生命周期状态
    pub lifecycle_status: EndpointLifecycleStatus,

    /// 版本号
    pub version: u32,

    /// 安全配置
    pub security: EndpointSecurity,

    /// 自定义属性
    pub properties: HashMap<String, String>,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 更新时间
    pub updated_at: DateTime<Utc>,

    /// 创建人
    pub created_by: Option<String>,

    /// 更新人
    pub updated_by: Option<String>,
}

impl Endpoint {
    /// 创建新端点
    pub fn new(
        id: impl Into<EndpointId>,
        organization_id: impl Into<OrganizationId>,
        system_id: impl Into<SystemId>,
        name: impl Into<String>,
        system_type: MedicalSystemType,
        protocol: ProtocolType,
        connection: ConnectionConfig,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            organization_id: organization_id.into(),
            system_id: system_id.into(),
            roles: vec![EndpointRole::Consumer],
            name: name.into(),
            description: None,
            system_type,
            protocol,
            connection,
            auth: None,
            config: EndpointConfig::default(),
            enabled: true,
            lifecycle_status: EndpointLifecycleStatus::Active,
            version: 1,
            security: EndpointSecurity::default(),
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
            created_by: None,
            updated_by: None,
        }
    }

    pub fn with_roles(mut self, roles: Vec<EndpointRole>) -> Self {
        self.roles = roles;
        self
    }

    /// 设置认证配置
    pub fn with_auth(mut self, auth: AuthConfig) -> Self {
        self.auth = Some(auth);
        self
    }

    /// 设置端点配置
    pub fn with_config(mut self, config: EndpointConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置安全配置
    pub fn with_security(mut self, security: EndpointSecurity) -> Self {
        self.security = security;
        self
    }

    /// 添加属性
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// 获取完整连接地址
    pub fn address(&self) -> String {
        self.connection.address()
    }

    /// 是否需要认证
    pub fn requires_auth(&self) -> bool {
        self.auth.is_some()
    }
}

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// 主机地址
    pub host: String,

    /// 端口
    pub port: u16,

    /// 路径（HTTP/REST）
    pub path: Option<String>,

    /// 是否启用 TLS
    pub tls_enabled: bool,

    /// TLS 证书路径
    pub tls_cert_path: Option<String>,

    /// 连接超时（秒）
    pub connect_timeout_secs: u64,

    /// 读取超时（秒）
    pub read_timeout_secs: u64,

    /// 写入超时（秒）
    pub write_timeout_secs: u64,

    /// 连接池大小
    pub pool_size: u32,

    /// 重连间隔（秒）
    pub reconnect_interval_secs: u64,

    /// 心跳间隔（秒）
    pub keepalive_secs: Option<u64>,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("HSB_DEFAULT_ENDPOINT_HOST")
                .unwrap_or_else(|_| "gateway.internal".to_string()),
            port: 80,
            path: None,
            tls_enabled: false,
            tls_cert_path: None,
            connect_timeout_secs: 10,
            read_timeout_secs: 30,
            write_timeout_secs: 30,
            pool_size: 10,
            reconnect_interval_secs: 5,
            keepalive_secs: Some(60),
        }
    }
}

impl ConnectionConfig {
    /// 创建 HTTP 连接配置
    pub fn http(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            ..Default::default()
        }
    }

    /// 创建 TCP 连接配置
    pub fn tcp(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            connect_timeout_secs: 10,
            read_timeout_secs: 60,
            write_timeout_secs: 60,
            pool_size: 5,
            ..Default::default()
        }
    }

    /// 获取地址字符串
    pub fn address(&self) -> String {
        let scheme = if self.tls_enabled { "https" } else { "http" };
        match &self.path {
            Some(path) => format!("{}://{}:{}{}", scheme, self.host, self.port, path),
            None => format!("{}://{}:{}", scheme, self.host, self.port),
        }
    }

    /// 获取连接超时
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.connect_timeout_secs)
    }

    /// 获取读取超时
    pub fn read_timeout(&self) -> Duration {
        Duration::from_secs(self.read_timeout_secs)
    }

    /// 获取写入超时
    pub fn write_timeout(&self) -> Duration {
        Duration::from_secs(self.write_timeout_secs)
    }
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// 无认证
    None,

    /// 基本认证
    Basic { username: String, password: String },

    /// Bearer Token
    Bearer { token: String },

    /// API Key
    ApiKey { key: String, header_name: String },

    /// OAuth2 客户端凭证
    OAuth2ClientCredentials {
        client_id: String,
        client_secret: String,
        token_url: String,
        scope: Option<String>,
    },

    /// 证书认证
    Certificate {
        cert_path: String,
        key_path: String,
        ca_path: Option<String>,
    },

    /// SSO Token（集成 rust-sso）
    SsoToken {
        sso_endpoint: String,
        app_id: String,
    },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::None
    }
}

impl AuthConfig {
    /// 创建基本认证
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// 创建 Bearer Token 认证
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer {
            token: token.into(),
        }
    }

    /// 创建 API Key 认证
    pub fn api_key(key: impl Into<String>, header_name: impl Into<String>) -> Self {
        Self::ApiKey {
            key: key.into(),
            header_name: header_name.into(),
        }
    }
}

/// 端点配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    /// 最大重试次数
    pub max_retries: u32,

    /// 重试间隔（毫秒）
    pub retry_interval_ms: u64,

    /// 是否启用压缩
    pub compression_enabled: bool,

    /// 最大消息大小（字节）
    pub max_message_size: usize,

    /// 并发限制
    pub concurrency_limit: u32,

    /// 速率限制（每秒请求数）
    pub rate_limit: Option<u32>,

    /// 熔断阈值
    pub circuit_breaker_threshold: Option<u32>,

    /// 是否记录请求/响应体
    pub log_body: bool,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_interval_ms: 1000,
            compression_enabled: false,
            max_message_size: 10 * 1024 * 1024, // 10MB
            concurrency_limit: 100,
            rate_limit: None,
            circuit_breaker_threshold: Some(5),
            log_body: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_creation() {
        let endpoint = Endpoint::new(
            "HIS_ENDPOINT_01",
            "ORG_HOSPITAL_01",
            "HIS_01",
            "医院信息系统",
            MedicalSystemType::His,
            ProtocolType::Http,
            ConnectionConfig::http("192.168.1.100", 8080),
        )
        .with_auth(AuthConfig::basic("admin", "password"));

        assert_eq!(endpoint.id.as_str(), "HIS_ENDPOINT_01");
        assert_eq!(endpoint.organization_id.as_str(), "ORG_HOSPITAL_01");
        assert_eq!(endpoint.system_id.as_str(), "HIS_01");
        assert!(endpoint.requires_auth());
        assert_eq!(endpoint.address(), "http://192.168.1.100:8080");
        assert_eq!(endpoint.version, 1);
    }
}
