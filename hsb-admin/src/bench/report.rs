//! 报告生成模块

use super::stats::{BenchStats, SuiteStats};

/// 报告生成器
pub struct ReportGenerator {
    suites: Vec<SuiteStats>,
}

impl ReportGenerator {
    pub fn new(suites: Vec<SuiteStats>) -> Self {
        Self { suites }
    }

    /// 打印文本格式报告
    pub fn print_text(&self) {
        for suite in &self.suites {
            println!();
            println!("━━━ {} ━━━", suite.name);
            println!();

            // 打印表头
            println!(
                "{:<30} {:>10} {:>10} {:>12} {:>12} {:>12} {:>12} {:>12}",
                "测试名称",
                "迭代次数",
                "成功率",
                "平均(μs)",
                "P50(μs)",
                "P99(μs)",
                "Min(μs)",
                "吞吐量"
            );
            println!("{}", "-".repeat(120));

            for t in &suite.tests {
                let success_rate = format!("{:.1}%", t.success_rate());
                let status = if t.failures == 0 { "✓" } else { "✗" };

                println!(
                    "{} {:<28} {:>10} {:>10} {:>12.2} {:>12.2} {:>12.2} {:>12.2} {:>12.0}",
                    status,
                    t.name,
                    t.iterations,
                    success_rate,
                    t.avg_duration_us,
                    t.p50_us,
                    t.p99_us,
                    t.min_us,
                    t.throughput
                );
            }

            println!();
            println!(
                "  ▸ 测试总数: {} | 通过: {} | 失败: {} | 耗时: {:.3}s",
                suite.total_tests,
                suite.passed_tests,
                suite.failed_tests,
                suite.total_duration_secs
            );
        }
    }

    /// 生成 JSON 格式报告
    pub fn to_json(&self) -> String {
        #[derive(serde::Serialize)]
        struct Report {
            generated_at: String,
            suites: Vec<SuiteStats>,
            summary: Summary,
        }

        #[derive(serde::Serialize)]
        struct Summary {
            total_suites: usize,
            total_tests: usize,
            total_passed: usize,
            total_failed: usize,
            overall_success_rate: f64,
        }

        let total_tests: usize = self.suites.iter().map(|s| s.total_tests).sum();
        let total_passed: usize = self.suites.iter().map(|s| s.passed_tests).sum();
        let total_failed: usize = self.suites.iter().map(|s| s.failed_tests).sum();

        let report = Report {
            generated_at: chrono::Utc::now().to_rfc3339(),
            suites: self.suites.clone(),
            summary: Summary {
                total_suites: self.suites.len(),
                total_tests,
                total_passed,
                total_failed,
                overall_success_rate: if total_tests > 0 {
                    total_passed as f64 / total_tests as f64 * 100.0
                } else {
                    0.0
                },
            },
        };

        serde_json::to_string_pretty(&report).unwrap_or_default()
    }

    /// 生成 CSV 格式报告
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();
        csv.push_str("Suite,Test,Iterations,Successes,Failures,SuccessRate,AvgUs,P50Us,P90Us,P99Us,MinUs,MaxUs,StdDevUs,Throughput\n");

        for suite in &self.suites {
            for test in &suite.tests {
                csv.push_str(&format!(
                    "{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}\n",
                    suite.name,
                    test.name,
                    test.iterations,
                    test.successes,
                    test.failures,
                    test.success_rate(),
                    test.avg_duration_us,
                    test.p50_us,
                    test.p90_us,
                    test.p99_us,
                    test.min_us,
                    test.max_us,
                    test.std_dev_us,
                    test.throughput
                ));
            }
        }

        csv
    }

    /// 打印总结
    pub fn print_summary(&self) {
        let total_suites = self.suites.len();
        let total_tests: usize = self.suites.iter().map(|s| s.total_tests).sum();
        let total_passed: usize = self.suites.iter().map(|s| s.passed_tests).sum();
        let total_failed: usize = self.suites.iter().map(|s| s.failed_tests).sum();
        let total_iterations: usize = self
            .suites
            .iter()
            .flat_map(|s| &s.tests)
            .map(|t| t.iterations)
            .sum();

        let all_stats: Vec<&BenchStats> = self.suites.iter().flat_map(|s| &s.tests).collect();

        // 计算整体 P99
        let avg_p99 = if !all_stats.is_empty() {
            all_stats.iter().map(|s| s.p99_us).sum::<f64>() / all_stats.len() as f64
        } else {
            0.0
        };

        let avg_throughput = if !all_stats.is_empty() {
            all_stats.iter().map(|s| s.throughput).sum::<f64>() / all_stats.len() as f64
        } else {
            0.0
        };

        println!("═══════════════════════════════════════════════════════════════");
        println!("                        总结报告");
        println!("═══════════════════════════════════════════════════════════════");
        println!();
        println!("  ▸ 测试套件数: {}", total_suites);
        println!("  ▸ 测试用例数: {}", total_tests);
        println!("  ▸ 总迭代次数: {}", total_iterations);
        println!("  ▸ 通过用例数: {}", total_passed);
        println!("  ▸ 失败用例数: {}", total_failed);
        println!(
            "  ▸ 整体成功率: {:.2}%",
            if total_tests > 0 {
                total_passed as f64 / total_tests as f64 * 100.0
            } else {
                0.0
            }
        );
        println!("  ▸ 平均 P99 延迟: {:.2} μs", avg_p99);
        println!("  ▸ 平均吞吐量: {:.0} ops/s", avg_throughput);
        println!();

        // 找出最慢和最快的测试
        if let Some(slowest) = all_stats
            .iter()
            .max_by(|a, b| a.p99_us.partial_cmp(&b.p99_us).unwrap())
        {
            println!(
                "  ▸ 最慢测试 (P99): {} ({:.2} μs)",
                slowest.name, slowest.p99_us
            );
        }
        if let Some(fastest) = all_stats
            .iter()
            .min_by(|a, b| a.p99_us.partial_cmp(&b.p99_us).unwrap())
        {
            println!(
                "  ▸ 最快测试 (P99): {} ({:.2} μs)",
                fastest.name, fastest.p99_us
            );
        }

        println!();
        println!("═══════════════════════════════════════════════════════════════");
    }
}
