//! 指标收集

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// 指标收集器
pub struct MetricsCollector {
    counters: Arc<RwLock<HashMap<String, Arc<AtomicU64>>>>,
    gauges: Arc<RwLock<HashMap<String, Arc<AtomicU64>>>>,
    histograms: Arc<RwLock<HashMap<String, Histogram>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(RwLock::new(HashMap::new())),
            gauges: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 增加计数器
    pub async fn increment(&self, name: &str, value: u64) {
        let counters = self.counters.read().await;
        if let Some(counter) = counters.get(name) {
            counter.fetch_add(value, Ordering::Relaxed);
            return;
        }
        drop(counters);

        let mut counters = self.counters.write().await;
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        counter.fetch_add(value, Ordering::Relaxed);
    }

    /// 设置 gauge
    pub async fn set_gauge(&self, name: &str, value: u64) {
        let gauges = self.gauges.read().await;
        if let Some(gauge) = gauges.get(name) {
            gauge.store(value, Ordering::Relaxed);
            return;
        }
        drop(gauges);

        let mut gauges = self.gauges.write().await;
        let gauge = gauges
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
        gauge.store(value, Ordering::Relaxed);
    }

    /// 记录直方图值
    pub async fn observe(&self, name: &str, value: f64) {
        let histograms = self.histograms.read().await;
        if let Some(histogram) = histograms.get(name) {
            histogram.observe(value);
            return;
        }
        drop(histograms);

        let mut histograms = self.histograms.write().await;
        let histogram = histograms
            .entry(name.to_string())
            .or_insert_with(Histogram::new);
        histogram.observe(value);
    }

    /// 获取所有指标
    pub async fn all_metrics(&self) -> Metrics {
        let mut metrics = Metrics::default();

        let counters = self.counters.read().await;
        for (name, value) in counters.iter() {
            metrics
                .counters
                .insert(name.clone(), value.load(Ordering::Relaxed));
        }

        let gauges = self.gauges.read().await;
        for (name, value) in gauges.iter() {
            metrics
                .gauges
                .insert(name.clone(), value.load(Ordering::Relaxed));
        }

        let histograms = self.histograms.read().await;
        for (name, histogram) in histograms.iter() {
            metrics.histograms.insert(name.clone(), histogram.summary());
        }

        metrics
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// 简单直方图
pub struct Histogram {
    values: Arc<RwLock<Vec<f64>>>,
    count: AtomicU64,
    sum: Arc<RwLock<f64>>,
}

impl Histogram {
    pub fn new() -> Self {
        Self {
            values: Arc::new(RwLock::new(Vec::new())),
            count: AtomicU64::new(0),
            sum: Arc::new(RwLock::new(0.0)),
        }
    }

    pub fn observe(&self, value: f64) {
        // 使用 blocking 操作，因为这是内部方法
        if let Ok(mut values) = self.values.try_write() {
            values.push(value);
            self.count.fetch_add(1, Ordering::Relaxed);
        }
        if let Ok(mut sum) = self.sum.try_write() {
            *sum += value;
        }
    }

    pub fn summary(&self) -> HistogramSummary {
        let values = match self.values.try_read() {
            Ok(v) => v.clone(),
            Err(_) => return HistogramSummary::default(),
        };
        let sum = self.sum.try_read().map(|s| *s).unwrap_or(0.0);
        let count = self.count.load(Ordering::Relaxed);

        if values.is_empty() {
            return HistogramSummary::default();
        }

        let mut sorted = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = sorted.len();
        HistogramSummary {
            count,
            sum,
            mean: sum / (count as f64),
            min: sorted.first().copied().unwrap_or(0.0),
            max: sorted.last().copied().unwrap_or(0.0),
            p50: sorted.get(len * 50 / 100).copied().unwrap_or(0.0),
            p90: sorted.get(len * 90 / 100).copied().unwrap_or(0.0),
            p95: sorted.get(len * 95 / 100).copied().unwrap_or(0.0),
            p99: sorted.get(len * 99 / 100).copied().unwrap_or(0.0),
        }
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

/// 直方图摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistogramSummary {
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

/// 所有指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub counters: HashMap<String, u64>,
    pub gauges: HashMap<String, u64>,
    pub histograms: HashMap<String, HistogramSummary>,
}

/// 预定义指标名称
pub mod metric_names {
    // 消息计数
    pub const MESSAGES_RECEIVED: &str = "hsb_messages_received_total";
    pub const MESSAGES_SENT: &str = "hsb_messages_sent_total";
    pub const MESSAGES_FAILED: &str = "hsb_messages_failed_total";
    pub const MESSAGES_RETRIED: &str = "hsb_messages_retried_total";
    pub const MESSAGES_DEAD_LETTERED: &str = "hsb_messages_dead_lettered_total";

    // 按协议
    pub const MESSAGES_BY_PROTOCOL: &str = "hsb_messages_by_protocol";
    pub const MESSAGES_BY_SOURCE: &str = "hsb_messages_by_source";
    pub const MESSAGES_BY_TARGET: &str = "hsb_messages_by_target";

    // 延迟
    pub const PROCESSING_LATENCY: &str = "hsb_processing_latency_ms";
    pub const DISPATCH_LATENCY: &str = "hsb_dispatch_latency_ms";
    pub const END_TO_END_LATENCY: &str = "hsb_end_to_end_latency_ms";

    // 队列
    pub const QUEUE_SIZE: &str = "hsb_queue_size";
    pub const DLQ_SIZE: &str = "hsb_dlq_size";

    // 连接
    pub const ACTIVE_CONNECTIONS: &str = "hsb_active_connections";
    pub const CONNECTION_ERRORS: &str = "hsb_connection_errors_total";

    // 熔断器
    pub const CIRCUIT_BREAKER_OPEN: &str = "hsb_circuit_breaker_open";
    pub const CIRCUIT_BREAKER_TRIPS: &str = "hsb_circuit_breaker_trips_total";
}

/// 指标标签
#[derive(Debug, Clone, Default)]
pub struct MetricLabels {
    labels: HashMap<String, String>,
}

impl MetricLabels {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(mut self, key: &str, value: &str) -> Self {
        self.labels.insert(key.to_string(), value.to_string());
        self
    }

    pub fn to_suffix(&self) -> String {
        if self.labels.is_empty() {
            return String::new();
        }

        let pairs: Vec<String> = self
            .labels
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        format!("{{{}}}", pairs.join(","))
    }
}
