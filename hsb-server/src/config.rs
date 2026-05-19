//! 服务器配置

use serde::{Deserialize, Serialize};
use std::{env, path::Path};

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器基本配置
    pub server: ServerSettings,
    /// HTTP 服务配置
    pub http: HttpSettings,
    /// TCP/MLLP 服务配置
    pub tcp: TcpSettings,
    /// gRPC 服务配置
    pub grpc: GrpcSettings,
    /// 数据库配置
    pub database: DatabaseSettings,
    /// 缓存配置（PG UNLOGGED 表）
    pub cache: CacheSettings,
    /// 消息队列配置
    pub mq: MqSettings,
    /// Kafka 配置
    pub kafka: KafkaSettings,
    /// NATS 配置
    pub nats: NatsSettings,
    /// 持久化配置
    pub persistence: PersistenceSettings,
    /// SSO 集成配置
    pub sso: SsoSettings,
    /// 审计配置
    pub audit: AuditSettings,
    /// 可靠性配置
    pub reliability: ReliabilitySettings,
    /// 日志配置
    pub logging: LoggingSettings,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings::default(),
            http: HttpSettings::default(),
            tcp: TcpSettings::default(),
            grpc: GrpcSettings::default(),
            database: DatabaseSettings::default(),
            cache: CacheSettings::default(),
            mq: MqSettings::default(),
            kafka: KafkaSettings::default(),
            nats: NatsSettings::default(),
            persistence: PersistenceSettings::default(),
            sso: SsoSettings::default(),
            audit: AuditSettings::default(),
            reliability: ReliabilitySettings::default(),
            logging: LoggingSettings::default(),
        }
    }
}

impl ServerConfig {
    /// 从文件加载配置
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let path = Path::new(path);

        if !path.exists() {
            let mut config = Self::default();
            config.apply_env_overrides();
            return Ok(config);
        }

        let content = std::fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&content)?;
        config.apply_env_overrides();
        config.validate_security()?;
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        self.database.url = env_string("HSB_DATABASE_URL", &self.database.url);
        self.mq.enabled = env_bool("HSB_MQ_ENABLED", self.mq.enabled);
        self.mq.host = env_string("HSB_MQ_HOST", &self.mq.host);
        self.mq.port = env_u16("HSB_MQ_PORT", self.mq.port);
        self.mq.username = env_string("HSB_MQ_USERNAME", &self.mq.username);
        self.mq.password = env_string("HSB_MQ_PASSWORD", &self.mq.password);
        self.mq.vhost = env_string("HSB_MQ_VHOST", &self.mq.vhost);
        self.kafka.enabled = env_bool("HSB_KAFKA_ENABLED", self.kafka.enabled);
        self.kafka.bootstrap_servers =
            env_string("HSB_KAFKA_BOOTSTRAP_SERVERS", &self.kafka.bootstrap_servers);
        self.kafka.client_id = env_string("HSB_KAFKA_CLIENT_ID", &self.kafka.client_id);
        self.kafka.default_topic =
            env_optional_string("HSB_KAFKA_DEFAULT_TOPIC", self.kafka.default_topic.clone());
        self.kafka.security_protocol =
            env_string("HSB_KAFKA_SECURITY_PROTOCOL", &self.kafka.security_protocol);
        self.kafka.sasl_username =
            env_optional_string("HSB_KAFKA_SASL_USERNAME", self.kafka.sasl_username.clone());
        self.kafka.sasl_password =
            env_optional_string("HSB_KAFKA_SASL_PASSWORD", self.kafka.sasl_password.clone());
        self.kafka.sasl_mechanism = env_optional_string(
            "HSB_KAFKA_SASL_MECHANISM",
            self.kafka.sasl_mechanism.clone(),
        );
        self.kafka.consumer.group_id =
            env_string("HSB_KAFKA_GROUP_ID", &self.kafka.consumer.group_id);
        self.kafka.consumer.topics = env_csv("HSB_KAFKA_TOPICS", &self.kafka.consumer.topics);
        self.http.route_prefix =
            normalize_route_prefix(&env_string("HSB_ROUTE_PREFIX", &self.http.route_prefix));
        self.nats.urls = env_csv("HSB_NATS_URLS", &self.nats.urls);
        self.sso.enabled = env_bool("HSB_SSO_ENABLED", self.sso.enabled);
        self.sso.endpoint = env_string("RUST_SSO_GRPC_ENDPOINT", &self.sso.endpoint);
        self.sso.web_base_url = env_string("RUST_SSO_WEB_BASE_URL", &self.sso.web_base_url);
        self.sso.client_id = env_string("RUST_SSO_CLIENT_ID", &self.sso.client_id);
        self.sso.client_secret = env_string("RUST_SSO_CLIENT_SECRET", &self.sso.client_secret);
        self.sso.callback_url = env_string("HSB_SSO_CALLBACK_URL", &self.sso.callback_url);
        self.sso.scope = env_string("HSB_SSO_SCOPE", &self.sso.scope);
    }

    fn validate_security(&self) -> anyhow::Result<()> {
        if !self.server.environment.eq_ignore_ascii_case("production") {
            return Ok(());
        }

        if self.sso.enabled && is_placeholder_secret(&self.sso.client_secret) {
            anyhow::bail!("production SSO requires RUST_SSO_CLIENT_SECRET to be set to a non-placeholder secret");
        }

        if self.mq.enabled
            && self.mq.username == "guest"
            && is_placeholder_secret(&self.mq.password)
        {
            anyhow::bail!("production MQ requires non-default HSB_MQ_USERNAME/HSB_MQ_PASSWORD");
        }

        if self.persistence.postgres_enabled && database_url_uses_placeholder_secret(&self.database.url) {
            anyhow::bail!("production database URL contains a placeholder or default password");
        }

        Ok(())
    }
}

/// 服务器基本设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    /// 服务名称
    pub name: String,
    /// 实例 ID
    pub instance_id: Option<String>,
    /// 环境
    pub environment: String,
    /// 最大并发数
    pub max_concurrency: usize,
    /// 关闭超时（秒）
    pub shutdown_timeout_secs: u64,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            name: "hsb-server".to_string(),
            instance_id: None,
            environment: "development".to_string(),
            max_concurrency: 1000,
            shutdown_timeout_secs: 30,
        }
    }
}

/// HTTP 服务设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSettings {
    /// 是否启用
    pub enabled: bool,
    /// 监听地址
    pub listen_address: String,
    /// HTTP 端口
    pub port: u16,
    /// HTTP 路由前缀，例如 /hsb；空字符串表示根路径
    #[serde(default)]
    pub route_prefix: String,
    /// Admin API 端口
    pub admin_port: u16,
    /// 启用 TLS
    pub tls_enabled: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// 最大请求体大小（字节）
    pub max_body_size: usize,
}

impl Default for HttpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: "0.0.0.0".to_string(),
            port: 8080,
            route_prefix: normalize_route_prefix(&env_string("HSB_ROUTE_PREFIX", "")),
            admin_port: 8081,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout_secs: 30,
            max_body_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// TCP/MLLP 服务设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpSettings {
    /// 是否启用
    pub enabled: bool,
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub port: u16,
    /// 最大连接数
    pub max_connections: usize,
    /// 连接超时（秒）
    pub connection_timeout_secs: u64,
    /// 读取超时（秒）
    pub read_timeout_secs: u64,
    /// 写入超时（秒）
    pub write_timeout_secs: u64,
}

impl Default for TcpSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: "0.0.0.0".to_string(),
            port: 2575,
            max_connections: 100,
            connection_timeout_secs: 30,
            read_timeout_secs: 60,
            write_timeout_secs: 30,
        }
    }
}

/// gRPC 服务设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcSettings {
    /// 是否启用
    pub enabled: bool,
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub port: u16,
    /// 最大并发流
    pub max_concurrent_streams: u32,
    /// 启用 TLS
    pub tls_enabled: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
}

impl Default for GrpcSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            listen_address: "0.0.0.0".to_string(),
            port: 10051,
            max_concurrent_streams: 100,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

/// 数据库设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
    /// 数据库 URL
    pub url: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 最小连接数
    pub min_connections: u32,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 空闲超时（秒）
    pub idle_timeout_secs: u64,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            url: env_string(
                "HSB_DATABASE_URL",
                "postgres://postgres:postgres@postgres:5432/hsb",
            ),
            max_connections: 10,
            min_connections: 2,
            connect_timeout_secs: 30,
            idle_timeout_secs: 600,
        }
    }
}

/// 缓存设置（使用 PostgreSQL UNLOGGED 表 + JSONB）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
    /// 是否启用缓存
    pub enabled: bool,
    /// 默认过期时间（秒）
    pub default_ttl_secs: u64,
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl_secs: 300,
            cleanup_interval_secs: 60,
        }
    }
}

/// 消息队列设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqSettings {
    /// 是否启用
    pub enabled: bool,
    /// 主机地址
    pub host: String,
    /// 端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 虚拟主机
    pub vhost: String,
    /// 消费者预取数
    pub prefetch_count: u16,
    /// 队列前缀
    pub queue_prefix: String,
}

impl Default for MqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            host: env_string("HSB_MQ_HOST", "rabbitmq"),
            port: 5672,
            username: "guest".to_string(),
            password: "guest".to_string(),
            vhost: "/".to_string(),
            prefetch_count: 10,
            queue_prefix: "hsb".to_string(),
        }
    }
}

/// Kafka 设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaSettings {
    /// 是否启用
    pub enabled: bool,
    /// broker 列表
    pub bootstrap_servers: String,
    /// 客户端 ID
    pub client_id: String,
    /// 默认 topic
    pub default_topic: Option<String>,
    /// 安全协议
    pub security_protocol: String,
    /// SASL 用户名
    pub sasl_username: Option<String>,
    /// SASL 密码
    pub sasl_password: Option<String>,
    /// SASL 机制
    pub sasl_mechanism: Option<String>,
    /// socket 超时（秒）
    pub socket_timeout_secs: u64,
    /// 发送超时（秒）
    pub message_timeout_secs: u64,
    /// 消费者配置
    pub consumer: KafkaConsumerSettings,
}

impl Default for KafkaSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            bootstrap_servers: env_string("HSB_KAFKA_BOOTSTRAP_SERVERS", "kafka:9092"),
            client_id: env_string("HSB_KAFKA_CLIENT_ID", "hsb-server"),
            default_topic: env::var("HSB_KAFKA_DEFAULT_TOPIC").ok(),
            security_protocol: env_string("HSB_KAFKA_SECURITY_PROTOCOL", "PLAINTEXT"),
            sasl_username: env::var("HSB_KAFKA_SASL_USERNAME").ok(),
            sasl_password: env::var("HSB_KAFKA_SASL_PASSWORD").ok(),
            sasl_mechanism: env::var("HSB_KAFKA_SASL_MECHANISM").ok(),
            socket_timeout_secs: env_u64("HSB_KAFKA_SOCKET_TIMEOUT_SECS", 30),
            message_timeout_secs: env_u64("HSB_KAFKA_MESSAGE_TIMEOUT_SECS", 30),
            consumer: KafkaConsumerSettings::default(),
        }
    }
}

/// Kafka 消费者设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConsumerSettings {
    /// 消费组 ID
    pub group_id: String,
    /// 订阅 topics
    pub topics: Vec<String>,
    /// 是否从最早消息开始消费
    pub start_from_earliest: bool,
    /// session timeout（秒）
    pub session_timeout_secs: u64,
}

impl Default for KafkaConsumerSettings {
    fn default() -> Self {
        Self {
            group_id: env_string("HSB_KAFKA_GROUP_ID", "hsb-server-group"),
            topics: env_csv("HSB_KAFKA_TOPICS", &[]),
            start_from_earliest: env_bool("HSB_KAFKA_START_FROM_EARLIEST", false),
            session_timeout_secs: env_u64("HSB_KAFKA_SESSION_TIMEOUT_SECS", 30),
        }
    }
}

/// SSO 集成设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoSettings {
    /// 是否启用
    pub enabled: bool,
    /// SSO gRPC 端点
    pub endpoint: String,
    /// SSO Web 基地址
    pub web_base_url: String,
    /// 客户端 ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// OAuth 回调地址
    pub callback_url: String,
    /// OAuth scope
    pub scope: String,
    /// 令牌缓存时间（秒）
    pub token_cache_secs: u64,
}

impl Default for SsoSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: env_string("RUST_SSO_GRPC_ENDPOINT", "http://rust-sso:50051"),
            web_base_url: env_string("RUST_SSO_WEB_BASE_URL", "http://rust-sso:8099"),
            client_id: env_string("RUST_SSO_CLIENT_ID", ""),
            client_secret: env_string("RUST_SSO_CLIENT_SECRET", ""),
            callback_url: env_string(
                "HSB_SSO_CALLBACK_URL",
                "http://hsb-server:8080/auth/callback",
            ),
            scope: env_string("HSB_SSO_SCOPE", "openid profile email"),
            token_cache_secs: 300,
        }
    }
}

/// 审计设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSettings {
    /// 是否启用
    pub enabled: bool,
    /// 记录消息内容
    pub log_message_content: bool,
    /// 脱敏敏感数据
    pub mask_sensitive_data: bool,
    /// 保留天数
    pub retention_days: u32,
    /// 批量写入大小
    pub batch_size: usize,
}

impl Default for AuditSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            log_message_content: false,
            mask_sensitive_data: true,
            retention_days: 90,
            batch_size: 100,
        }
    }
}

/// 可靠性设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReliabilitySettings {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试基础延迟（毫秒）
    pub retry_base_delay_ms: u64,
    /// 启用死信队列
    pub enable_dlq: bool,
    /// 熔断器错误阈值
    pub circuit_breaker_threshold: u32,
    /// 熔断器恢复时间（秒）
    pub circuit_breaker_recovery_secs: u64,
}

impl Default for ReliabilitySettings {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_base_delay_ms: 1000,
            enable_dlq: true,
            circuit_breaker_threshold: 5,
            circuit_breaker_recovery_secs: 60,
        }
    }
}

/// 日志设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// 日志级别
    pub level: String,
    /// JSON 格式
    pub json_format: bool,
    /// 日志文件路径
    pub file_path: Option<String>,
    /// 最大文件大小（MB）
    pub max_file_size_mb: u64,
    /// 最大文件数
    pub max_files: u32,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file_path: Some("logs/hsb.log".to_string()),
            max_file_size_mb: 100,
            max_files: 10,
        }
    }
}

/// NATS 设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsSettings {
    /// 是否启用
    pub enabled: bool,
    /// NATS 服务器地址列表
    pub urls: Vec<String>,
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// Token 认证
    pub token: Option<String>,
    /// Subject 前缀
    pub subject_prefix: String,
    /// 启用 JetStream
    pub jetstream_enabled: bool,
    /// JetStream 默认流名称
    pub jetstream_stream: String,
    /// JetStream 流 Subject
    pub jetstream_subjects: Vec<String>,
    /// 消息最大保留时间（秒）
    pub max_age_secs: u64,
}

impl Default for NatsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            urls: env_csv("HSB_NATS_URLS", &["nats://nats:4222".to_string()]),
            username: None,
            password: None,
            token: None,
            subject_prefix: "hsb".to_string(),
            jetstream_enabled: true,
            jetstream_stream: "HSB_MESSAGES".to_string(),
            jetstream_subjects: vec!["hsb.>".to_string()],
            max_age_secs: 604800,
        }
    }
}

/// 持久化设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceSettings {
    /// Redb 本地存储路径
    pub redb_path: String,
    /// 是否启用 Redb
    pub redb_enabled: bool,
    /// 是否启用 PostgreSQL
    pub postgres_enabled: bool,
}

impl Default for PersistenceSettings {
    fn default() -> Self {
        Self {
            redb_path: "data/hsb_local.redb".to_string(),
            redb_enabled: true,
            postgres_enabled: true,
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_optional_string(key: &str, default: Option<String>) -> Option<String> {
    env::var(key).ok().or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_u16(key: &str, default: u16) -> u16 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn is_placeholder_secret(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "change-me" | "changeme" | "guest" | "password" | "secret" | "default"
        )
        || normalized.starts_with("replace_with")
        || normalized.starts_with("replace-with")
}

fn database_url_uses_placeholder_secret(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    normalized.contains(":change-me@")
        || normalized.contains(":changeme@")
        || normalized.contains(":postgres@")
        || normalized.contains(":password@")
}

fn env_csv(key: &str, default: &[String]) -> Vec<String> {
    env::var(key)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| default.to_vec())
}

fn normalize_route_prefix(value: &str) -> String {
    let trimmed = value.trim().trim_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("/{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::ServerConfig;

    #[test]
    fn production_rejects_placeholder_sso_secret() {
        let mut config = ServerConfig::default();
        config.server.environment = "production".to_string();
        config.sso.enabled = true;
        config.sso.client_secret = "change-me".to_string();

        assert!(config.validate_security().is_err());
    }

    #[test]
    fn production_rejects_default_mq_credentials() {
        let mut config = ServerConfig::default();
        config.server.environment = "production".to_string();
        config.sso.enabled = false;
        config.mq.enabled = true;
        config.mq.username = "guest".to_string();
        config.mq.password = "guest".to_string();

        assert!(config.validate_security().is_err());
    }
}
