//! 消息队列传输配置

use serde::{Deserialize, Serialize};
use std::env;

/// 消息队列传输配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqTransportConfig {
    /// 传输名称
    pub name: String,
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
    /// 预取数量
    pub prefetch_count: u16,
    /// 是否使用 TLS
    pub use_tls: bool,
    /// 心跳间隔（秒）
    pub heartbeat_secs: u16,
    /// 连接超时（秒）
    pub connection_timeout_secs: u64,
}

impl Default for MqTransportConfig {
    fn default() -> Self {
        Self {
            name: "rabbitmq".to_string(),
            host: env_string("HSB_MQ_HOST", "rabbitmq"),
            port: 5672,
            username: "guest".to_string(),
            password: "guest".to_string(),
            vhost: "/".to_string(),
            prefetch_count: 10,
            use_tls: false,
            heartbeat_secs: 60,
            connection_timeout_secs: 30,
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

impl MqTransportConfig {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            ..Default::default()
        }
    }

    pub fn with_credentials(mut self, username: &str, password: &str) -> Self {
        self.username = username.to_string();
        self.password = password.to_string();
        self
    }

    pub fn with_vhost(mut self, vhost: &str) -> Self {
        self.vhost = vhost.to_string();
        self
    }

    pub fn with_tls(mut self) -> Self {
        self.use_tls = true;
        self.port = 5671; // 默认 TLS 端口
        self
    }
}

/// 队列配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// 队列名称
    pub name: String,
    /// 是否持久化
    pub durable: bool,
    /// 是否排他
    pub exclusive: bool,
    /// 是否自动删除
    pub auto_delete: bool,
    /// 死信 Exchange
    pub dead_letter_exchange: Option<String>,
    /// 死信路由键
    pub dead_letter_routing_key: Option<String>,
    /// 消息 TTL（毫秒）
    pub message_ttl_ms: Option<u32>,
    /// 最大长度
    pub max_length: Option<u32>,
    /// 最大大小（字节）
    pub max_length_bytes: Option<u64>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            durable: true,
            exclusive: false,
            auto_delete: false,
            dead_letter_exchange: None,
            dead_letter_routing_key: None,
            message_ttl_ms: None,
            max_length: None,
            max_length_bytes: None,
        }
    }
}

impl QueueConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn with_dlx(mut self, exchange: &str, routing_key: &str) -> Self {
        self.dead_letter_exchange = Some(exchange.to_string());
        self.dead_letter_routing_key = Some(routing_key.to_string());
        self
    }

    pub fn with_ttl(mut self, ttl_ms: u32) -> Self {
        self.message_ttl_ms = Some(ttl_ms);
        self
    }
}

/// Exchange 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    /// Exchange 名称
    pub name: String,
    /// Exchange 类型
    pub exchange_type: ExchangeType,
    /// 是否持久化
    pub durable: bool,
    /// 是否自动删除
    pub auto_delete: bool,
}

/// Exchange 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExchangeType {
    Direct,
    Fanout,
    Topic,
    Headers,
}

impl ExchangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Fanout => "fanout",
            Self::Topic => "topic",
            Self::Headers => "headers",
        }
    }
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            exchange_type: ExchangeType::Direct,
            durable: true,
            auto_delete: false,
        }
    }
}

impl ExchangeConfig {
    pub fn direct(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exchange_type: ExchangeType::Direct,
            ..Default::default()
        }
    }

    pub fn topic(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exchange_type: ExchangeType::Topic,
            ..Default::default()
        }
    }

    pub fn fanout(name: &str) -> Self {
        Self {
            name: name.to_string(),
            exchange_type: ExchangeType::Fanout,
            ..Default::default()
        }
    }
}
