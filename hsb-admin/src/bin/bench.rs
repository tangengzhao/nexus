//! HSB 基准测试套件
//!
//! 测试所有 HSB 功能，记录性能指标

use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use hsb_admin::bench::bench_runner::BenchRunner;
use hsb_admin::bench::report::ReportGenerator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let args: Vec<String> = env::args().collect();

    let iterations: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);

    let suite = args.get(2).map(|s| s.as_str()).unwrap_or("all");

    let format = args.get(3).map(|s| s.as_str()).unwrap_or("text");

    let verbose = args.iter().any(|s| s == "-v" || s == "--verbose");

    // 初始化日志
    if verbose {
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║      HSB (Hospital Service Bus) 基准测试套件                   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("用法: hsb-bench [iterations] [suite] [format] [-v]");
    println!("  iterations: 每个测试运行次数 (默认: 200)");
    println!("  suite: all, core, adapter, transport, reliability, audit, engine");
    println!("  format: text, json, csv");
    println!();
    println!("配置:");
    println!("  • 每个测试运行次数: {}", iterations);
    println!("  • 测试套件: {}", suite);
    println!("  • 输出格式: {}", format);
    println!();

    // 创建基准测试运行器
    let mut runner = BenchRunner::new(iterations, 1);

    // 根据选择运行测试套件
    match suite {
        "all" => {
            runner.run_core_tests().await;
            runner.run_adapter_tests().await;
            runner.run_transport_tests().await;
            runner.run_reliability_tests().await;
            runner.run_audit_tests().await;
            runner.run_engine_tests().await;
        }
        "core" => runner.run_core_tests().await,
        "adapter" => runner.run_adapter_tests().await,
        "transport" => runner.run_transport_tests().await,
        "reliability" => runner.run_reliability_tests().await,
        "audit" => runner.run_audit_tests().await,
        "engine" => runner.run_engine_tests().await,
        _ => {
            eprintln!("错误: 未知测试套件 '{}'", suite);
            std::process::exit(1);
        }
    }

    // 生成报告
    let report = ReportGenerator::new(runner.results());

    match format {
        "text" => report.print_text(),
        "json" => {
            let json = report.to_json();
            println!("{}", json);
        }
        "csv" => {
            let csv = report.to_csv();
            println!("{}", csv);
        }
        _ => report.print_text(),
    }

    // 打印总结
    println!();
    report.print_summary();

    Ok(())
}
