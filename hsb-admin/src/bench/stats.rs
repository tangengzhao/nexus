//! 统计计算模块

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 测试结果统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchStats {
    /// 测试名称
    pub name: String,
    /// 测试套件
    pub suite: String,
    /// 总运行次数
    pub iterations: usize,
    /// 成功次数
    pub successes: usize,
    /// 失败次数
    pub failures: usize,
    /// 平均耗时 (微秒)
    pub avg_duration_us: f64,
    /// P50 延迟 (微秒)
    pub p50_us: f64,
    /// P90 延迟 (微秒)
    pub p90_us: f64,
    /// P99 延迟 (微秒)
    pub p99_us: f64,
    /// 最小耗时 (微秒)
    pub min_us: f64,
    /// 最大耗时 (微秒)
    pub max_us: f64,
    /// 标准差 (微秒)
    pub std_dev_us: f64,
    /// 吞吐量 (ops/sec)
    pub throughput: f64,
}

impl BenchStats {
    /// 从运行时间列表计算统计数据
    pub fn calculate(name: &str, suite: &str, durations: &[Duration], failures: usize) -> Self {
        let iterations = durations.len();
        let successes = iterations.saturating_sub(failures);

        if iterations == 0 {
            return Self {
                name: name.to_string(),
                suite: suite.to_string(),
                iterations: 0,
                successes: 0,
                failures,
                avg_duration_us: 0.0,
                p50_us: 0.0,
                p90_us: 0.0,
                p99_us: 0.0,
                min_us: 0.0,
                max_us: 0.0,
                std_dev_us: 0.0,
                throughput: 0.0,
            };
        }

        // 转换为微秒
        let mut times_us: Vec<f64> = durations.iter().map(|d| d.as_micros() as f64).collect();
        times_us.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let sum: f64 = times_us.iter().sum();
        let avg = sum / iterations as f64;

        // 计算百分位数
        let p50_idx = (iterations as f64 * 0.50) as usize;
        let p90_idx = (iterations as f64 * 0.90) as usize;
        let p99_idx = (iterations as f64 * 0.99) as usize;

        let p50 = times_us
            .get(p50_idx.min(iterations - 1))
            .copied()
            .unwrap_or(0.0);
        let p90 = times_us
            .get(p90_idx.min(iterations - 1))
            .copied()
            .unwrap_or(0.0);
        let p99 = times_us
            .get(p99_idx.min(iterations - 1))
            .copied()
            .unwrap_or(0.0);

        let min = times_us.first().copied().unwrap_or(0.0);
        let max = times_us.last().copied().unwrap_or(0.0);

        // 计算标准差
        let variance: f64 =
            times_us.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / iterations as f64;
        let std_dev = variance.sqrt();

        // 计算吞吐量 (ops/sec)
        let throughput = if avg > 0.0 { 1_000_000.0 / avg } else { 0.0 };

        Self {
            name: name.to_string(),
            suite: suite.to_string(),
            iterations,
            successes,
            failures,
            avg_duration_us: avg,
            p50_us: p50,
            p90_us: p90,
            p99_us: p99,
            min_us: min,
            max_us: max,
            std_dev_us: std_dev,
            throughput,
        }
    }

    /// 成功率
    pub fn success_rate(&self) -> f64 {
        if self.iterations == 0 {
            0.0
        } else {
            self.successes as f64 / self.iterations as f64 * 100.0
        }
    }
}

/// 测试套件统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteStats {
    /// 套件名称
    pub name: String,
    /// 测试总数
    pub total_tests: usize,
    /// 通过测试数
    pub passed_tests: usize,
    /// 失败测试数
    pub failed_tests: usize,
    /// 总运行时间 (秒)
    pub total_duration_secs: f64,
    /// 套件内所有测试的统计
    pub tests: Vec<BenchStats>,
}

impl SuiteStats {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            total_duration_secs: 0.0,
            tests: Vec::new(),
        }
    }

    pub fn add_test(&mut self, stats: BenchStats) {
        if stats.failures == 0 {
            self.passed_tests += 1;
        } else {
            self.failed_tests += 1;
        }
        self.total_tests += 1;
        self.total_duration_secs += (stats.avg_duration_us * stats.iterations as f64) / 1_000_000.0;
        self.tests.push(stats);
    }
}
