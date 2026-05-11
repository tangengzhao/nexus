//! NATS 传输配置

use serde::{Deserialize, Serialize};
use std::env;

/// NATS 传输配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsTransportConfig {
    /// 传输名称
    pub name: String,
    /// NATS 服务器地址列表
    pub urls: Vec<String>,
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// Token 认证
    pub token: Option<String>,
    /// NKey 凭证文件路径
    pub credentials_path: Option<String>,
    /// Subject 前缀（用于隔离不同环境）
    pub subject_prefix: String,
    /// Ping 间隔（秒）
    pub ping_interval_secs: u64,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// JetStream 配置
    pub jetstream: JetStreamConfig,
}

impl Default for NatsTransportConfig {
    fn default() -> Self {
        Self {
            name: "nats".to_string(),
            urls: env_csv("HSB_NATS_URLS", &["nats://nats:4222".to_string()]),
            username: None,
            password: None,
            token: None,
            credentials_path: None,
            subject_prefix: "hsb".to_string(),
            ping_interval_secs: 30,
            request_timeout_secs: 10,
            jetstream: JetStreamConfig::default(),
        }
    }
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

/// JetStream 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JetStreamConfig {
    /// 是否启用 JetStream
    pub enabled: bool,
    /// 默认流名称
    pub default_stream: String,
    /// 流的 Subject 过滤（例如 "hsb.>"）
    pub stream_subjects: Vec<String>,
    /// 消息保留策略
    pub retention: RetentionPolicy,
    /// 存储类型
    pub storage: StorageType,
    /// 最大消息数
    pub max_messages: i64,
    /// 最大字节数
    pub max_bytes: i64,
    /// 消息最大保留时间（秒）
    pub max_age_secs: u64,
    /// 副本数
    pub num_replicas: usize,
    /// 去重窗口（秒，用于 ExactlyOnce 语义）
    pub dedup_window_secs: u64,
}

impl Default for JetStreamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_stream: "HSB_MESSAGES".to_string(),
            stream_subjects: vec!["hsb.>".to_string()],
            retention: RetentionPolicy::Limits,
            storage: StorageType::File,
            max_messages: -1,     // 不限制
            max_bytes: -1,        // 不限制
            max_age_secs: 604800, // 7 天
            num_replicas: 1,
            dedup_window_secs: 120, // 2 分钟去重窗口
        }
    }
}

/// 消息保留策略
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    /// 基于限制（默认）
    Limits,
    /// 基于 Interest（有消费者才保留）
    Interest,
    /// 工作队列（消费后删除）
    WorkQueue,
}

/// 存储类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    /// 文件存储
    File,
    /// 内存存储
    Memory,
}
