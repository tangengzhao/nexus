//! gRPC 配置

use serde::{Deserialize, Serialize};
use std::env;

/// gRPC 传输配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcTransportConfig {
    /// gRPC 端点地址
    pub endpoint: String,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// 是否使用 TLS
    pub use_tls: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
    /// CA 证书路径
    pub tls_ca_path: Option<String>,
    /// 启用压缩
    pub enable_compression: bool,
    /// 最大消息大小（字节）
    pub max_message_size: usize,
    /// 连接池大小
    pub pool_size: usize,
    /// 启用负载均衡
    pub enable_load_balancing: bool,
    /// 重试配置
    pub retry_config: GrpcRetryConfig,
}

impl Default for GrpcTransportConfig {
    fn default() -> Self {
        Self {
            endpoint: env_string("HSB_GRPC_ENDPOINT", "http://grpc-service:50051"),
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            use_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            tls_ca_path: None,
            enable_compression: true,
            max_message_size: 4 * 1024 * 1024, // 4MB
            pool_size: 10,
            enable_load_balancing: false,
            retry_config: GrpcRetryConfig::default(),
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

impl GrpcTransportConfig {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            ..Default::default()
        }
    }

    pub fn with_tls(mut self, cert_path: &str, key_path: &str, ca_path: Option<&str>) -> Self {
        self.use_tls = true;
        self.tls_cert_path = Some(cert_path.to_string());
        self.tls_key_path = Some(key_path.to_string());
        self.tls_ca_path = ca_path.map(String::from);
        self
    }

    pub fn with_timeout(mut self, connect_secs: u64, request_secs: u64) -> Self {
        self.connect_timeout_secs = connect_secs;
        self.request_timeout_secs = request_secs;
        self
    }

    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }
}

/// gRPC 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcRetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始退避时间（毫秒）
    pub initial_backoff_ms: u64,
    /// 最大退避时间（毫秒）
    pub max_backoff_ms: u64,
    /// 退避乘数
    pub backoff_multiplier: f64,
    /// 可重试的状态码
    pub retryable_codes: Vec<i32>,
}

impl Default for GrpcRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 10000,
            backoff_multiplier: 2.0,
            retryable_codes: vec![
                14, // UNAVAILABLE
                4,  // DEADLINE_EXCEEDED
                8,  // RESOURCE_EXHAUSTED
            ],
        }
    }
}

/// gRPC 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcServerConfig {
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub port: u16,
    /// 使用 TLS
    pub use_tls: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
    /// 最大并发流
    pub max_concurrent_streams: u32,
    /// 最大连接数
    pub max_connections: usize,
    /// 启用健康检查服务
    pub enable_health_check: bool,
    /// 启用反射服务
    pub enable_reflection: bool,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            port: 50051,
            use_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
            max_concurrent_streams: 100,
            max_connections: 1000,
            enable_health_check: true,
            enable_reflection: true,
        }
    }
}

impl GrpcServerConfig {
    pub fn address(&self) -> String {
        format!("{}:{}", self.listen_address, self.port)
    }
}
