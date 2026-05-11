//! 基准测试运行器

use std::time::Instant;

use super::stats::{BenchStats, SuiteStats};
use super::test_suites::*;

/// 基准测试运行器
pub struct BenchRunner {
    /// 每个测试的迭代次数
    iterations: usize,
    /// 并发数
    #[allow(dead_code)]
    concurrency: usize,
    /// 测试套件结果
    results: Vec<SuiteStats>,
}

impl BenchRunner {
    pub fn new(iterations: usize, concurrency: usize) -> Self {
        Self {
            iterations,
            concurrency,
            results: Vec::new(),
        }
    }

    /// 获取测试结果
    pub fn results(self) -> Vec<SuiteStats> {
        self.results
    }

    /// 运行单个测试并收集统计
    async fn run_test<F, Fut>(&self, name: &str, test_fn: F) -> BenchStats
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let mut durations = Vec::with_capacity(self.iterations);
        let mut failures = 0;

        print!("  测试 {} ... ", name);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        for _ in 0..self.iterations {
            let start = Instant::now();
            let result = test_fn().await;
            let elapsed = start.elapsed();

            if result.is_err() {
                failures += 1;
            }
            durations.push(elapsed);
        }

        let stats = BenchStats::calculate(name, "", &durations, failures);

        if failures == 0 {
            println!(
                "✓ 通过 (avg: {:.2}μs, p99: {:.2}μs, throughput: {:.0} ops/s)",
                stats.avg_duration_us, stats.p99_us, stats.throughput
            );
        } else {
            println!("✗ 失败 ({}/{} 失败)", failures, self.iterations);
        }

        stats
    }

    /// 运行核心模块测试
    pub async fn run_core_tests(&mut self) {
        println!();
        println!("▶ 运行核心模块测试 (hsb-core)");
        println!();

        let mut suite = SuiteStats::new("核心模块 (hsb-core)");

        // 消息创建测试
        let stats = self
            .run_test("消息创建 (MessageBuilder)", || async {
                core_tests::test_message_creation()
            })
            .await;
        suite.add_test(stats);

        // 消息序列化测试
        let stats = self
            .run_test("消息序列化 (JSON)", || async {
                core_tests::test_message_serialization()
            })
            .await;
        suite.add_test(stats);

        // 消息反序列化测试
        let stats = self
            .run_test("消息反序列化 (JSON)", || async {
                core_tests::test_message_deserialization()
            })
            .await;
        suite.add_test(stats);

        // 消息元数据操作测试
        let stats = self
            .run_test("消息元数据操作", || async {
                core_tests::test_metadata_operations()
            })
            .await;
        suite.add_test(stats);

        // 路由规则创建测试
        let stats = self
            .run_test("路由规则创建", || async {
                core_tests::test_route_creation()
            })
            .await;
        suite.add_test(stats);

        // 路由匹配测试
        let stats = self
            .run_test("路由匹配", || async {
                core_tests::test_route_matching()
            })
            .await;
        suite.add_test(stats);

        // 转换器链测试
        let stats = self
            .run_test("转换器链执行", || async {
                core_tests::test_transformer_chain()
            })
            .await;
        suite.add_test(stats);

        // 处理上下文测试
        let stats = self
            .run_test("处理上下文操作", || async {
                core_tests::test_message_context().await
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }

    /// 运行协议适配器测试
    pub async fn run_adapter_tests(&mut self) {
        println!();
        println!("▶ 运行协议适配器测试 (hsb-adapter-*)");
        println!();

        let mut suite = SuiteStats::new("协议适配器 (hsb-adapter)");

        // HL7 解析测试
        let stats = self
            .run_test("HL7 v2.x 消息解析", || async {
                adapter_tests::test_hl7_parsing().await
            })
            .await;
        suite.add_test(stats);

        // HL7 序列化测试
        let stats = self
            .run_test("HL7 v2.x 消息序列化", || async {
                adapter_tests::test_hl7_serialization().await
            })
            .await;
        suite.add_test(stats);

        // HL7 验证测试
        let stats = self
            .run_test("HL7 v2.x 消息验证", || async {
                adapter_tests::test_hl7_validation().await
            })
            .await;
        suite.add_test(stats);

        // FHIR 解析测试
        let stats = self
            .run_test("FHIR R5 资源解析", || async {
                adapter_tests::test_fhir_parsing().await
            })
            .await;
        suite.add_test(stats);

        // FHIR 序列化测试
        let stats = self
            .run_test("FHIR R5 资源序列化", || async {
                adapter_tests::test_fhir_serialization().await
            })
            .await;
        suite.add_test(stats);

        // DICOM 解析测试
        let stats = self
            .run_test("DICOM 消息解析", || async {
                adapter_tests::test_dicom_parsing().await
            })
            .await;
        suite.add_test(stats);

        // SOAP 解析测试
        let stats = self
            .run_test("SOAP 消息解析", || async {
                adapter_tests::test_soap_parsing().await
            })
            .await;
        suite.add_test(stats);

        // SOAP 序列化测试
        let stats = self
            .run_test("SOAP 消息序列化", || async {
                adapter_tests::test_soap_serialization().await
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }

    /// 运行传输层测试
    pub async fn run_transport_tests(&mut self) {
        println!();
        println!("▶ 运行传输层测试 (hsb-transport-*)");
        println!();

        let mut suite = SuiteStats::new("传输层 (hsb-transport)");

        // 传输配置测试
        let stats = self
            .run_test("传输配置创建", || async {
                transport_tests::test_transport_config()
            })
            .await;
        suite.add_test(stats);

        // 传输注册表测试
        let stats = self
            .run_test("传输注册表操作", || async {
                transport_tests::test_transport_registry()
            })
            .await;
        suite.add_test(stats);

        // 请求/响应构建测试
        let stats = self
            .run_test("请求响应构建", || async {
                transport_tests::test_request_response()
            })
            .await;
        suite.add_test(stats);

        // 连接池测试
        let stats = self
            .run_test("连接池模拟", || async {
                transport_tests::test_connection_pool()
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }

    /// 运行可靠性层测试
    pub async fn run_reliability_tests(&mut self) {
        println!();
        println!("▶ 运行可靠性层测试 (hsb-reliability)");
        println!();

        let mut suite = SuiteStats::new("可靠性层 (hsb-reliability)");

        // 内存队列测试
        let stats = self
            .run_test("内存队列入队", || async {
                reliability_tests::test_queue_enqueue().await
            })
            .await;
        suite.add_test(stats);

        // 内存队列出队测试
        let stats = self
            .run_test("内存队列出队", || async {
                reliability_tests::test_queue_dequeue().await
            })
            .await;
        suite.add_test(stats);

        // 重试策略测试
        let stats = self
            .run_test("重试策略计算", || async {
                reliability_tests::test_retry_strategy()
            })
            .await;
        suite.add_test(stats);

        // 熔断器测试
        let stats = self
            .run_test("熔断器状态转换", || async {
                reliability_tests::test_circuit_breaker().await
            })
            .await;
        suite.add_test(stats);

        // 死信队列测试
        let stats = self
            .run_test("死信队列操作", || async {
                reliability_tests::test_dlq_operations().await
            })
            .await;
        suite.add_test(stats);

        // 消息状态追踪测试
        let stats = self
            .run_test("消息状态追踪", || async {
                reliability_tests::test_message_status()
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }

    /// 运行审计层测试
    pub async fn run_audit_tests(&mut self) {
        println!();
        println!("▶ 运行审计层测试 (hsb-audit)");
        println!();

        let mut suite = SuiteStats::new("审计层 (hsb-audit)");

        // 审计事件创建测试
        let stats = self
            .run_test("审计事件创建", || async {
                audit_tests::test_audit_event_creation()
            })
            .await;
        suite.add_test(stats);

        // 审计事件存储测试
        let stats = self
            .run_test("审计事件存储", || async {
                audit_tests::test_audit_storage()
            })
            .await;
        suite.add_test(stats);

        // 消息追踪测试
        let stats = self
            .run_test("消息追踪记录", || async {
                audit_tests::test_message_tracing()
            })
            .await;
        suite.add_test(stats);

        // 指标收集测试
        let stats = self
            .run_test("指标收集", || async {
                audit_tests::test_metrics_collection().await
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }

    /// 运行路由引擎测试
    pub async fn run_engine_tests(&mut self) {
        println!();
        println!("▶ 运行路由引擎测试 (hsb-engine)");
        println!();

        let mut suite = SuiteStats::new("路由引擎 (hsb-engine)");

        // 路由器创建测试
        let stats = self
            .run_test("路由器创建", || async {
                engine_tests::test_router_creation().await
            })
            .await;
        suite.add_test(stats);

        // 路由规则添加测试
        let stats = self
            .run_test("路由规则添加", || async {
                engine_tests::test_route_addition().await
            })
            .await;
        suite.add_test(stats);

        // 路由查找测试
        let stats = self
            .run_test("路由查找", || async {
                engine_tests::test_route_finding().await
            })
            .await;
        suite.add_test(stats);

        // 分发结果测试
        let stats = self
            .run_test("分发结果构建", || async {
                engine_tests::test_dispatch_result()
            })
            .await;
        suite.add_test(stats);

        // 处理管道测试
        let stats = self
            .run_test("处理管道执行", || async {
                engine_tests::test_processing_pipeline().await
            })
            .await;
        suite.add_test(stats);

        // 适配器注册表测试
        let stats = self
            .run_test("适配器注册表", || async {
                engine_tests::test_adapter_registry()
            })
            .await;
        suite.add_test(stats);

        self.results.push(suite);
    }
}
