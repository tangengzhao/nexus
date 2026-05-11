//! HTTP 传输配置

use hsb_core::ConnectionPoolConfig;
use serde::{Deserialize, Serialize};

/// HTTP 传输配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTransportConfig {
    /// 传输名称
    pub name: String,
    /// 基础 URL
    pub base_url: Option<String>,
    /// 是否使用 TLS
    pub use_tls: bool,
    /// 超时（秒）
    pub timeout_secs: u64,
    /// 连接池配置
    pub pool_config: ConnectionPoolConfig,
    /// 代理
    pub proxy: Option<String>,
    /// 是否禁用证书验证
    pub disable_certificate_validation: bool,
    /// 健康检查 URL
    pub health_check_url: Option<String>,
    /// 默认请求头
    pub default_headers: Vec<(String, String)>,
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self {
            name: "http".to_string(),
            base_url: None,
            use_tls: false,
            timeout_secs: 30,
            pool_config: ConnectionPoolConfig::default(),
            proxy: None,
            disable_certificate_validation: false,
            health_check_url: None,
            default_headers: Vec::new(),
        }
    }
}

impl HttpTransportConfig {
    pub fn https() -> Self {
        Self {
            name: "https".to_string(),
            use_tls: true,
            ..Default::default()
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = Some(url.to_string());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn with_health_check(mut self, url: &str) -> Self {
        self.health_check_url = Some(url.to_string());
        self
    }
}
