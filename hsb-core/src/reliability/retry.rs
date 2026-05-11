//! 重试机制

use hsb_common::{HsbError, HsbResult};
use std::future::Future;
use std::time::Duration;
use tracing::{info, warn};

/// 重试策略
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// 固定延迟
    Fixed { delay_ms: u64 },
    /// 指数退避
    Exponential {
        base_delay_ms: u64,
        max_delay_ms: u64,
        multiplier: f64,
    },
    /// 线性增长
    Linear {
        initial_delay_ms: u64,
        increment_ms: u64,
        max_delay_ms: u64,
    },
    /// 自定义延迟序列
    Custom { delays_ms: Vec<u64> },
}

impl RetryStrategy {
    /// 计算第 n 次重试的延迟
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let ms = match self {
            Self::Fixed { delay_ms } => *delay_ms,
            Self::Exponential {
                base_delay_ms,
                max_delay_ms,
                multiplier,
            } => {
                let delay = (*base_delay_ms as f64) * multiplier.powi(attempt as i32);
                (delay as u64).min(*max_delay_ms)
            }
            Self::Linear {
                initial_delay_ms,
                increment_ms,
                max_delay_ms,
            } => {
                let delay = initial_delay_ms + increment_ms * (attempt as u64);
                delay.min(*max_delay_ms)
            }
            Self::Custom { delays_ms } => delays_ms
                .get(attempt as usize)
                .copied()
                .unwrap_or_else(|| delays_ms.last().copied().unwrap_or(1000)),
        };

        Duration::from_millis(ms)
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::Exponential {
            base_delay_ms: 1000,
            max_delay_ms: 300000,
            multiplier: 2.0,
        }
    }
}

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试策略
    pub strategy: RetryStrategy,
    /// 是否添加抖动
    pub jitter: bool,
    /// 抖动因子 (0.0 - 1.0)
    pub jitter_factor: f64,
    /// 可重试的错误类型
    pub retryable_errors: Vec<String>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            strategy: RetryStrategy::default(),
            jitter: true,
            jitter_factor: 0.1,
            retryable_errors: Vec::new(),
        }
    }
}

impl RetryConfig {
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    pub fn with_strategy(mut self, strategy: RetryStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_jitter(mut self, enabled: bool, factor: f64) -> Self {
        self.jitter = enabled;
        self.jitter_factor = factor;
        self
    }
}

/// 重试执行器
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// 执行带重试的操作
    pub async fn execute<F, Fut, T>(&self, operation: F) -> HsbResult<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = HsbResult<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(result) => {
                    if attempt > 0 {
                        info!("Operation succeeded after {} retries", attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    if !self.is_retryable(&e) {
                        return Err(e);
                    }

                    if attempt < self.config.max_retries {
                        let delay = self.calculate_delay(attempt);
                        warn!(
                            "Operation failed (attempt {}), retrying in {:?}: {}",
                            attempt + 1,
                            delay,
                            e
                        );
                        tokio::time::sleep(delay).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| HsbError::InternalError {
            message: "Retry failed with unknown error".to_string(),
        }))
    }

    fn is_retryable(&self, error: &HsbError) -> bool {
        // 检查错误是否可重试
        error.is_retryable()
    }

    fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.config.strategy.delay_for_attempt(attempt);

        if self.config.jitter {
            let jitter_range = (base_delay.as_millis() as f64) * self.config.jitter_factor;
            let jitter = (rand_simple() * 2.0 - 1.0) * jitter_range;
            let final_ms = (base_delay.as_millis() as f64 + jitter).max(0.0) as u64;
            Duration::from_millis(final_ms)
        } else {
            base_delay
        }
    }
}

/// 简单随机数生成（避免依赖完整的 rand crate）
fn rand_simple() -> f64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
    );
    let hash = hasher.finish();
    (hash as f64) / (u64::MAX as f64)
}

/// 重试上下文
#[derive(Debug, Clone)]
pub struct RetryContext {
    /// 当前重试次数
    pub attempt: u32,
    /// 最大重试次数
    pub max_retries: u32,
    /// 累计延迟
    pub total_delay: Duration,
    /// 最后一次错误
    pub last_error: Option<String>,
}

impl RetryContext {
    pub fn new(max_retries: u32) -> Self {
        Self {
            attempt: 0,
            max_retries,
            total_delay: Duration::ZERO,
            last_error: None,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.attempt < self.max_retries
    }

    pub fn increment(&mut self, delay: Duration, error: Option<String>) {
        self.attempt += 1;
        self.total_delay += delay;
        self.last_error = error;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_strategy() {
        let strategy = RetryStrategy::Exponential {
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            multiplier: 2.0,
        };

        assert_eq!(strategy.delay_for_attempt(0), Duration::from_millis(1000));
        assert_eq!(strategy.delay_for_attempt(1), Duration::from_millis(2000));
        assert_eq!(strategy.delay_for_attempt(2), Duration::from_millis(4000));
        assert_eq!(strategy.delay_for_attempt(5), Duration::from_millis(30000)); // 被 max 限制
    }

    #[test]
    fn test_fixed_strategy() {
        let strategy = RetryStrategy::Fixed { delay_ms: 5000 };

        assert_eq!(strategy.delay_for_attempt(0), Duration::from_millis(5000));
        assert_eq!(strategy.delay_for_attempt(5), Duration::from_millis(5000));
    }
}
