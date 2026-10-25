//! 延时回复（Webhook）事务登记
//!
//! 路由选择“出口端延时回复”时，总线在向出口端投递的同时登记一条待回复事务；
//! 出口端稍后携带回复令牌（或关联 ID）回复，总线据此找到入口端的回调 URL 并转发。

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

/// 待回复事务
#[derive(Debug, Clone)]
pub struct PendingReply {
    /// 回复令牌（每次投递唯一）
    pub token: String,
    /// 业务关联 ID（入口消息的 correlation_id，缺省为消息 ID）
    pub correlation_id: String,
    /// 原始消息 ID
    pub message_id: String,
    /// 追踪 ID
    pub trace_id: String,
    /// 路由 ID
    pub route_id: String,
    /// 出口端点 ID
    pub target_endpoint: String,
    /// 入口端回调 URL
    pub callback_url: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
}

impl PendingReply {
    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }
}

/// 待回复事务登记表（内存，按过期时间自动清理）
#[derive(Debug, Default)]
pub struct WebhookRegistry {
    pending: Mutex<HashMap<String, PendingReply>>,
}

impl WebhookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记待回复事务
    pub fn register(&self, mut reply: PendingReply, ttl_secs: u64) {
        let ttl = ChronoDuration::seconds(i64::try_from(ttl_secs).unwrap_or(i64::MAX / 1000));
        reply.expires_at = reply
            .created_at
            .checked_add_signed(ttl)
            .unwrap_or(DateTime::<Utc>::MAX_UTC);
        let mut pending = self.lock();
        pending.retain(|_, item| !item.is_expired());
        pending.insert(reply.token.clone(), reply);
    }

    /// 取出（并移除）待回复事务。`key` 可以是回复令牌，或唯一对应的关联 ID。
    pub fn take(&self, key: &str) -> Option<PendingReply> {
        let mut pending = self.lock();
        pending.retain(|_, item| !item.is_expired());

        if let Some(reply) = pending.remove(key) {
            return Some(reply);
        }

        let mut matches = pending.values().filter(|item| item.correlation_id == key);
        let token = matches.next()?.token.clone();
        if matches.next().is_some() {
            // 关联 ID 对应多条待回复事务，无法确定归属，必须使用令牌
            return None;
        }
        pending.remove(&token)
    }

    /// 回调失败时放回，允许出口端重试
    pub fn restore(&self, reply: PendingReply) {
        self.lock().insert(reply.token.clone(), reply);
    }

    /// 撤销登记（投递失败时）
    pub fn remove(&self, token: &str) {
        self.lock().remove(token);
    }

    /// 当前未过期的待回复事务数
    pub fn len(&self) -> usize {
        let mut pending = self.lock();
        pending.retain(|_, item| !item.is_expired());
        pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, PendingReply>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(token: &str, correlation: &str) -> PendingReply {
        PendingReply {
            token: token.to_string(),
            correlation_id: correlation.to_string(),
            message_id: "m1".to_string(),
            trace_id: "t1".to_string(),
            route_id: "r1".to_string(),
            target_endpoint: "LIS".to_string(),
            callback_url: "http://localhost/cb".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now(),
        }
    }

    #[test]
    fn take_by_token_or_unique_correlation() {
        let registry = WebhookRegistry::new();
        registry.register(sample("tok1", "corr1"), 60);
        registry.register(sample("tok2", "corr2"), 60);
        assert_eq!(registry.len(), 2);

        assert_eq!(registry.take("corr1").unwrap().token, "tok1");
        assert_eq!(registry.take("tok2").unwrap().token, "tok2");
        assert!(registry.take("tok2").is_none());
    }

    #[test]
    fn ambiguous_correlation_requires_token() {
        let registry = WebhookRegistry::new();
        registry.register(sample("tok1", "corr"), 60);
        registry.register(sample("tok2", "corr"), 60);
        assert!(registry.take("corr").is_none());
        assert!(registry.take("tok1").is_some());
    }

    #[test]
    fn expired_entries_are_dropped() {
        let registry = WebhookRegistry::new();
        registry.register(sample("tok1", "corr1"), 0);
        assert!(registry.take("tok1").is_none());
        assert!(registry.is_empty());
    }
}
