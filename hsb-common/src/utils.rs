//! HSB 公共工具函数

use chrono::{DateTime, Utc};
use ulid::Ulid;

/// 生成 ULID（时间排序，单调递增）
pub fn generate_ulid() -> Ulid {
    Ulid::new()
}

/// 生成 ULID 字符串
pub fn generate_ulid_string() -> String {
    Ulid::new().to_string()
}

/// 获取当前 UTC 时间
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

/// 安全的字符串截断
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// 掩码敏感信息
pub fn mask_sensitive(s: &str, visible_chars: usize) -> String {
    if s.len() <= visible_chars * 2 {
        return "*".repeat(s.len());
    }
    let prefix = &s[..visible_chars];
    let suffix = &s[s.len() - visible_chars..];
    format!("{}***{}", prefix, suffix)
}

/// 安全解析整数
pub fn safe_parse_i64(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

/// 安全解析浮点数
pub fn safe_parse_f64(s: &str) -> Option<f64> {
    s.trim().parse().ok()
}

/// 生成追踪 ID
pub fn generate_trace_id() -> String {
    format!("hsb-{}", Ulid::new())
}

/// 计算指数退避延迟（毫秒）
pub fn exponential_backoff(attempt: u32, base_ms: u64, max_ms: u64) -> u64 {
    let delay = base_ms.saturating_mul(2u64.saturating_pow(attempt));
    delay.min(max_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
    }

    #[test]
    fn test_mask_sensitive() {
        assert_eq!(mask_sensitive("1234567890", 2), "12***90");
        assert_eq!(mask_sensitive("abc", 2), "***");
    }

    #[test]
    fn test_exponential_backoff() {
        assert_eq!(exponential_backoff(0, 100, 10000), 100);
        assert_eq!(exponential_backoff(1, 100, 10000), 200);
        assert_eq!(exponential_backoff(2, 100, 10000), 400);
        assert_eq!(exponential_backoff(10, 100, 10000), 10000); // capped
    }
}
