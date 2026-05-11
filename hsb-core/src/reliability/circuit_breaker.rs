//! 熔断器

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// 熔断器状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CircuitState {
    /// 关闭（正常状态）
    Closed,
    /// 打开（熔断中）
    Open,
    /// 半开（尝试恢复）
    HalfOpen,
}

/// 熔断器
pub struct CircuitBreaker {
    name: String,
    state: Arc<RwLock<CircuitState>>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: Arc<RwLock<Option<Instant>>>,
    config: CircuitBreakerConfig,
}

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 触发熔断的失败次数阈值
    pub failure_threshold: u32,
    /// 半开状态下成功次数阈值
    pub success_threshold: u32,
    /// 熔断持续时间（秒）
    pub open_duration_secs: u64,
    /// 失败计数窗口（秒）
    pub failure_window_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_duration_secs: 60,
            failure_window_secs: 60,
        }
    }
}

impl CircuitBreaker {
    pub fn new(name: &str, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.to_string(),
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> CircuitState {
        let mut state = self.state.write().await;

        // 检查是否应该从 Open 转换到 HalfOpen
        if *state == CircuitState::Open {
            let last_failure = self.last_failure_time.read().await;
            if let Some(time) = *last_failure {
                if time.elapsed() > Duration::from_secs(self.config.open_duration_secs) {
                    *state = CircuitState::HalfOpen;
                    self.success_count.store(0, Ordering::Relaxed);
                    info!("Circuit breaker '{}' transitioning to HalfOpen", self.name);
                }
            }
        }

        *state
    }

    /// 检查是否允许请求
    pub async fn allow_request(&self) -> bool {
        match self.state().await {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // 允许探测请求
        }
    }

    /// 记录成功
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                // 重置失败计数
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    self.failure_count.store(0, Ordering::Relaxed);
                    self.success_count.store(0, Ordering::Relaxed);
                    info!("Circuit breaker '{}' closed after recovery", self.name);
                }
            }
            CircuitState::Open => {
                // 不应该发生
            }
        }
    }

    /// 记录失败
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;

        // 更新最后失败时间
        {
            let mut last_failure = self.last_failure_time.write().await;
            *last_failure = Some(Instant::now());
        }

        match *state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= self.config.failure_threshold {
                    *state = CircuitState::Open;
                    warn!(
                        "Circuit breaker '{}' opened after {} failures",
                        self.name, count
                    );
                }
            }
            CircuitState::HalfOpen => {
                // 半开状态下的失败立即回到打开状态
                *state = CircuitState::Open;
                self.success_count.store(0, Ordering::Relaxed);
                warn!(
                    "Circuit breaker '{}' reopened after failure in HalfOpen state",
                    self.name
                );
            }
            CircuitState::Open => {
                // 已经打开，无需处理
            }
        }
    }

    /// 重置熔断器
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        info!("Circuit breaker '{}' manually reset", self.name);
    }

    /// 获取统计信息
    pub async fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            name: self.name.clone(),
            state: self.state().await,
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            failure_threshold: self.config.failure_threshold,
            success_threshold: self.config.success_threshold,
        }
    }
}

/// 熔断器统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub failure_threshold: u32,
    pub success_threshold: u32,
}

/// 熔断器注册表
pub struct CircuitBreakerRegistry {
    breakers: Arc<RwLock<std::collections::HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerRegistry {
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            default_config,
        }
    }

    /// 获取或创建熔断器
    pub async fn get_or_create(&self, name: &str) -> Arc<CircuitBreaker> {
        let breakers = self.breakers.read().await;
        if let Some(breaker) = breakers.get(name) {
            return breaker.clone();
        }
        drop(breakers);

        let mut breakers = self.breakers.write().await;
        // 双重检查
        if let Some(breaker) = breakers.get(name) {
            return breaker.clone();
        }

        let breaker = Arc::new(CircuitBreaker::new(name, self.default_config.clone()));
        breakers.insert(name.to_string(), breaker.clone());
        breaker
    }

    /// 获取所有熔断器统计
    pub async fn all_stats(&self) -> Vec<CircuitBreakerStats> {
        let breakers = self.breakers.read().await;
        let mut stats = Vec::new();

        for breaker in breakers.values() {
            stats.push(breaker.stats().await);
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_breaker_opens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.allow_request().await);

        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(!cb.allow_request().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_duration_secs: 0, // 立即进入半开
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // 等待进入半开状态
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
