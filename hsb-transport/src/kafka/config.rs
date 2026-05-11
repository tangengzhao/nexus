//! Kafka 传输配置。

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaTransportConfig {
    /// 传输名称
    pub name: String,
    /// Kafka broker 列表
    pub bootstrap_servers: String,
    /// 客户端标识
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
    pub consumer: KafkaConsumerConfig,
}

impl Default for KafkaTransportConfig {
    fn default() -> Self {
        Self {
            name: "kafka".to_string(),
            bootstrap_servers: env_string("HSB_KAFKA_BOOTSTRAP_SERVERS", "kafka:9092"),
            client_id: env_string("HSB_KAFKA_CLIENT_ID", "hsb-server"),
            default_topic: env::var("HSB_KAFKA_DEFAULT_TOPIC").ok(),
            security_protocol: env_string("HSB_KAFKA_SECURITY_PROTOCOL", "PLAINTEXT"),
            sasl_username: env::var("HSB_KAFKA_SASL_USERNAME").ok(),
            sasl_password: env::var("HSB_KAFKA_SASL_PASSWORD").ok(),
            sasl_mechanism: env::var("HSB_KAFKA_SASL_MECHANISM").ok(),
            socket_timeout_secs: env_u64("HSB_KAFKA_SOCKET_TIMEOUT_SECS", 30),
            message_timeout_secs: env_u64("HSB_KAFKA_MESSAGE_TIMEOUT_SECS", 30),
            consumer: KafkaConsumerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConsumerConfig {
    /// 消费组 ID
    pub group_id: String,
    /// 订阅 topics
    pub topics: Vec<String>,
    /// 从头消费
    pub start_from_earliest: bool,
    /// session timeout（秒）
    pub session_timeout_secs: u64,
}

impl Default for KafkaConsumerConfig {
    fn default() -> Self {
        Self {
            group_id: env_string("HSB_KAFKA_GROUP_ID", "hsb-server-group"),
            topics: env_csv("HSB_KAFKA_TOPICS", &[]),
            start_from_earliest: env_bool("HSB_KAFKA_START_FROM_EARLIEST", false),
            session_timeout_secs: env_u64("HSB_KAFKA_SESSION_TIMEOUT_SECS", 30),
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
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
