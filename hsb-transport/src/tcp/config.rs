//! TCP 传输配置

use serde::{Deserialize, Serialize};
use std::env;

/// TCP 传输配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTransportConfig {
    /// 传输名称
    pub name: String,
    /// 主机地址
    pub host: String,
    /// 端口
    pub port: u16,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 读写超时（秒）
    pub timeout_secs: u64,
    /// 是否使用 MLLP
    pub use_mllp: bool,
    /// 接收缓冲区大小
    pub buffer_size: usize,
    /// 是否启用 Keep-Alive
    pub keep_alive: bool,
    /// Keep-Alive 间隔（秒）
    pub keep_alive_interval_secs: u64,
    /// 是否启用 TCP_NODELAY
    pub no_delay: bool,
}

impl Default for TcpTransportConfig {
    fn default() -> Self {
        Self {
            name: "tcp".to_string(),
            host: env_string("HSB_TCP_HOST", "tcp-service"),
            port: 2575, // 标准 HL7 MLLP 端口
            connect_timeout_secs: 10,
            timeout_secs: 30,
            use_mllp: true,
            buffer_size: 65536,
            keep_alive: true,
            keep_alive_interval_secs: 30,
            no_delay: true,
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

impl TcpTransportConfig {
    pub fn mllp(host: &str, port: u16) -> Self {
        Self {
            name: format!("mllp-{}-{}", host, port),
            host: host.to_string(),
            port,
            use_mllp: true,
            ..Default::default()
        }
    }

    pub fn raw_tcp(host: &str, port: u16) -> Self {
        Self {
            name: format!("tcp-{}-{}", host, port),
            host: host.to_string(),
            port,
            use_mllp: false,
            ..Default::default()
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// TCP 服务端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpServerConfig {
    /// 绑定地址
    pub bind_addr: String,
    /// 绑定端口
    pub bind_port: u16,
    /// 是否使用 MLLP
    pub use_mllp: bool,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时（秒）
    pub connection_timeout_secs: u64,
    /// 接收缓冲区大小
    pub buffer_size: usize,
    /// 是否启用 TLS
    pub use_tls: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
}

impl Default for TcpServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            bind_port: 2575,
            use_mllp: true,
            max_connections: 100,
            connection_timeout_secs: 300,
            buffer_size: 65536,
            use_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}
