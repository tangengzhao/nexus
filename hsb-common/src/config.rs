//! HSB 配置定义

use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 监听地址
    pub host: String,
    /// 监听端口
    pub port: u16,
    /// 工作线程数
    pub workers: Option<usize>,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// 是否启用 TLS
    pub tls_enabled: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            workers: None,
            request_timeout_secs: 30,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

impl ServerConfig {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
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
    /// 是否启用 SSL
    pub ssl_mode: Option<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: env_string(
                "HSB_DATABASE_URL",
                "postgres://postgres:postgres@postgres:5432/hsb",
            ),
            max_connections: 20,
            min_connections: 5,
            connect_timeout_secs: 10,
            idle_timeout_secs: 300,
            ssl_mode: None,
        }
    }
}

impl DatabaseConfig {
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_secs(self.connect_timeout_secs)
    }

    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
    }
}

/// 缓存配置（使用 PostgreSQL UNLOGGED 表 + JSONB）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 是否启用缓存
    pub enabled: bool,
    /// 默认过期时间（秒）
    pub default_ttl_secs: u64,
    /// 清理间隔（秒）
    pub cleanup_interval_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_ttl_secs: 300,
            cleanup_interval_secs: 60,
        }
    }
}

/// 消息队列配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueConfig {
    /// MQ 类型
    pub mq_type: MqType,
    /// 连接 URL
    pub url: String,
    /// 消费者组
    pub consumer_group: Option<String>,
    /// 预取数量
    pub prefetch_count: u16,
}

/// MQ 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MqType {
    RabbitMq,
    Kafka,
    RobustMq,
}

impl Default for MessageQueueConfig {
    fn default() -> Self {
        Self {
            mq_type: MqType::RabbitMq,
            url: env_string("HSB_MQ_URL", "amqp://guest:guest@rabbitmq:5672"),
            consumer_group: None,
            prefetch_count: 10,
        }
    }
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 退避乘数
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay_ms: 1000,
            max_delay_ms: 60000,
            multiplier: 2.0,
        }
    }
}

/// SSO 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoClientConfig {
    /// SSO gRPC 地址
    pub grpc_endpoint: String,
    /// 应用 ID
    pub app_id: String,
    /// 应用密钥
    pub app_secret: String,
    /// 是否启用 TLS
    pub tls_enabled: bool,
    /// 连接超时（秒）
    pub timeout_secs: u64,
}

impl Default for SsoClientConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: env_string("RUST_SSO_GRPC_ENDPOINT", "http://rust-sso:50051"),
            app_id: "hsb".to_string(),
            app_secret: String::new(),
            tls_enabled: false,
            timeout_secs: 10,
        }
    }
}

/// etcd 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtcdConfig {
    /// etcd 端点列表
    pub endpoints: Vec<String>,
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// 键前缀
    pub key_prefix: String,
}

impl Default for EtcdConfig {
    fn default() -> Self {
        Self {
            endpoints: env_csv("HSB_ETCD_ENDPOINTS", &["http://etcd:2379".to_string()]),
            username: None,
            password: None,
            key_prefix: "/hsb/".to_string(),
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// 日志级别
    pub level: String,
    /// 日志格式
    pub format: LogFormat,
    /// 日志目录
    pub directory: Option<String>,
    /// 是否输出到控制台
    pub console: bool,
}

/// 日志格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Text,
    Json,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Json,
            directory: Some("logs".to_string()),
            console: true,
        }
    }
}

/// OpenTelemetry 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtelConfig {
    /// 是否启用
    pub enabled: bool,
    /// OTLP 端点
    pub endpoint: String,
    /// 服务名称
    pub service_name: String,
    /// 采样率
    pub sampling_ratio: f64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: env_string("HSB_OTEL_ENDPOINT", "http://otel-collector:4317"),
            service_name: "hsb".to_string(),
            sampling_ratio: 1.0,
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
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
