//! 死信队列

use crate::Message;
use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 死信消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetter {
    /// 原始消息
    pub message: Message,
    /// 死信原因
    pub reason: DeadLetterReason,
    /// 错误详情
    pub error_detail: String,
    /// 重试次数
    pub retry_count: u32,
    /// 最后处理时间
    pub last_processed_at: chrono::DateTime<chrono::Utc>,
    /// 进入死信队列时间
    pub dead_lettered_at: chrono::DateTime<chrono::Utc>,
    /// 源路由 ID
    pub source_route_id: Option<String>,
    /// 目标系统
    pub target_system: Option<String>,
}

/// 死信原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeadLetterReason {
    /// 超过最大重试次数
    MaxRetriesExceeded,
    /// 消息过期
    MessageExpired,
    /// 无法路由
    Unroutable,
    /// 目标系统不可用
    TargetUnavailable,
    /// 解析错误
    ParseError,
    /// 验证失败
    ValidationFailed,
    /// 转换失败
    TransformFailed,
    /// 手动移入
    ManuallyMoved,
    /// 其他
    Other(String),
}

impl DeadLetterReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::MaxRetriesExceeded => "MAX_RETRIES_EXCEEDED",
            Self::MessageExpired => "MESSAGE_EXPIRED",
            Self::Unroutable => "UNROUTABLE",
            Self::TargetUnavailable => "TARGET_UNAVAILABLE",
            Self::ParseError => "PARSE_ERROR",
            Self::ValidationFailed => "VALIDATION_FAILED",
            Self::TransformFailed => "TRANSFORM_FAILED",
            Self::ManuallyMoved => "MANUALLY_MOVED",
            Self::Other(_) => "OTHER",
        }
    }
}

/// 死信队列 Trait
#[async_trait]
pub trait DeadLetterQueue: Send + Sync {
    /// 添加死信
    async fn add(&self, dead_letter: DeadLetter) -> HsbResult<()>;

    /// 获取死信
    async fn get(&self, message_id: &str) -> HsbResult<Option<DeadLetter>>;

    /// 删除死信
    async fn delete(&self, message_id: &str) -> HsbResult<()>;

    /// 重新处理死信
    async fn reprocess(&self, message_id: &str) -> HsbResult<Message>;

    /// 列出死信
    async fn list(&self, filter: DeadLetterFilter) -> HsbResult<Vec<DeadLetter>>;

    /// 统计
    async fn stats(&self) -> HsbResult<DeadLetterStats>;

    /// 清理过期死信
    async fn cleanup(&self, older_than: chrono::DateTime<chrono::Utc>) -> HsbResult<usize>;
}

/// 死信过滤器
#[derive(Debug, Clone, Default)]
pub struct DeadLetterFilter {
    /// 原因
    pub reason: Option<DeadLetterReason>,
    /// 源系统
    pub source_system: Option<String>,
    /// 目标系统
    pub target_system: Option<String>,
    /// 开始时间
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 结束时间
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    /// 限制
    pub limit: Option<usize>,
    /// 偏移
    pub offset: Option<usize>,
}

/// 死信统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeadLetterStats {
    /// 总数
    pub total: u64,
    /// 按原因统计
    pub by_reason: HashMap<String, u64>,
    /// 按源系统统计
    pub by_source_system: HashMap<String, u64>,
    /// 按目标系统统计
    pub by_target_system: HashMap<String, u64>,
    /// 最早的死信时间
    pub oldest: Option<chrono::DateTime<chrono::Utc>>,
    /// 最新的死信时间
    pub newest: Option<chrono::DateTime<chrono::Utc>>,
}

/// 内存死信队列
pub struct InMemoryDlq {
    letters: Arc<RwLock<HashMap<String, DeadLetter>>>,
    max_size: usize,
}

impl InMemoryDlq {
    pub fn new(max_size: usize) -> Self {
        Self {
            letters: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }
}

#[async_trait]
impl DeadLetterQueue for InMemoryDlq {
    async fn add(&self, dead_letter: DeadLetter) -> HsbResult<()> {
        let mut letters = self.letters.write().await;

        if letters.len() >= self.max_size {
            // 删除最旧的
            if let Some(oldest_id) = letters
                .iter()
                .min_by_key(|(_, dl)| dl.dead_lettered_at)
                .map(|(id, _)| id.clone())
            {
                letters.remove(&oldest_id);
                warn!("DLQ full, removed oldest message: {}", oldest_id);
            }
        }

        let message_id = dead_letter.message.id.to_string();
        info!(
            "Adding message to DLQ: {} (reason: {:?})",
            message_id, dead_letter.reason
        );
        letters.insert(message_id, dead_letter);

        Ok(())
    }

    async fn get(&self, message_id: &str) -> HsbResult<Option<DeadLetter>> {
        let letters = self.letters.read().await;
        Ok(letters.get(message_id).cloned())
    }

    async fn delete(&self, message_id: &str) -> HsbResult<()> {
        let mut letters = self.letters.write().await;
        letters.remove(message_id);
        Ok(())
    }

    async fn reprocess(&self, message_id: &str) -> HsbResult<Message> {
        let mut letters = self.letters.write().await;
        let dead_letter = letters
            .remove(message_id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "DeadLetter".to_string(),
                id: message_id.to_string(),
            })?;

        info!("Reprocessing message from DLQ: {}", message_id);
        Ok(dead_letter.message)
    }

    async fn list(&self, filter: DeadLetterFilter) -> HsbResult<Vec<DeadLetter>> {
        let letters = self.letters.read().await;

        let mut result: Vec<_> = letters
            .values()
            .filter(|dl| {
                // 应用过滤条件
                if let Some(ref source) = filter.source_system {
                    if dl.message.source_system.to_string() != *source {
                        return false;
                    }
                }
                if let Some(ref target) = filter.target_system {
                    if dl.target_system.as_ref() != Some(target) {
                        return false;
                    }
                }
                if let Some(from) = filter.from_time {
                    if dl.dead_lettered_at < from {
                        return false;
                    }
                }
                if let Some(to) = filter.to_time {
                    if dl.dead_lettered_at > to {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // 按时间排序
        result.sort_by(|a, b| b.dead_lettered_at.cmp(&a.dead_lettered_at));

        // 应用分页
        let offset = filter.offset.unwrap_or(0);
        let limit = filter.limit.unwrap_or(100);

        Ok(result.into_iter().skip(offset).take(limit).collect())
    }

    async fn stats(&self) -> HsbResult<DeadLetterStats> {
        let letters = self.letters.read().await;

        let mut stats = DeadLetterStats {
            total: letters.len() as u64,
            ..Default::default()
        };

        for dl in letters.values() {
            // 按原因统计
            *stats
                .by_reason
                .entry(dl.reason.as_str().to_string())
                .or_insert(0) += 1;

            // 按源系统统计
            *stats
                .by_source_system
                .entry(dl.message.source_system.to_string())
                .or_insert(0) += 1;

            // 按目标系统统计
            if let Some(ref target) = dl.target_system {
                *stats.by_target_system.entry(target.clone()).or_insert(0) += 1;
            }

            // 时间范围
            match stats.oldest {
                None => stats.oldest = Some(dl.dead_lettered_at),
                Some(oldest) if dl.dead_lettered_at < oldest => {
                    stats.oldest = Some(dl.dead_lettered_at);
                }
                _ => {}
            }

            match stats.newest {
                None => stats.newest = Some(dl.dead_lettered_at),
                Some(newest) if dl.dead_lettered_at > newest => {
                    stats.newest = Some(dl.dead_lettered_at);
                }
                _ => {}
            }
        }

        Ok(stats)
    }

    async fn cleanup(&self, older_than: chrono::DateTime<chrono::Utc>) -> HsbResult<usize> {
        let mut letters = self.letters.write().await;
        let initial_len = letters.len();

        letters.retain(|_, dl| dl.dead_lettered_at >= older_than);

        let removed = initial_len - letters.len();
        if removed > 0 {
            info!("Cleaned up {} expired dead letters", removed);
        }

        Ok(removed)
    }
}

/// 创建死信
pub fn create_dead_letter(
    message: Message,
    reason: DeadLetterReason,
    error_detail: &str,
    retry_count: u32,
) -> DeadLetter {
    let now = chrono::Utc::now();
    DeadLetter {
        target_system: message.target_system.as_ref().map(|s| s.to_string()),
        message,
        reason,
        error_detail: error_detail.to_string(),
        retry_count,
        last_processed_at: now,
        dead_lettered_at: now,
        source_route_id: None,
    }
}
