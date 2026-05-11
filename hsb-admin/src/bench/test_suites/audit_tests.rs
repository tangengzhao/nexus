//! 审计层测试

use crate::audit::{
    AuditEvent, AuditEventBuilder, AuditEventType, AuditSeverity, MessageTrace, MetricsCollector,
    SpanStatus, TraceSpan, TraceStatus,
};
use hsb_common::ProtocolType;

/// 测试审计事件创建
pub fn test_audit_event_creation() -> Result<(), String> {
    // 使用 Builder 创建审计事件
    let event = AuditEventBuilder::new(AuditEventType::MessageReceived)
        .severity(AuditSeverity::Info)
        .source("HIS")
        .target("LIS")
        .message_id("MSG-001")
        .component("hl7-adapter")
        .description("ADT^A01 message received")
        .metadata("patient_id", serde_json::json!("P12345"))
        .duration(150)
        .success()
        .build();

    // 验证事件创建
    if event.event_type != AuditEventType::MessageReceived {
        return Err("事件类型不匹配".to_string());
    }

    if event.severity != AuditSeverity::Info {
        return Err("严重级别不匹配".to_string());
    }

    Ok(())
}

/// 测试审计事件序列化
pub fn test_audit_storage() -> Result<(), String> {
    let event = AuditEventBuilder::new(AuditEventType::MessageReceived)
        .severity(AuditSeverity::Info)
        .source("HIS")
        .target("LIS")
        .message_id("MSG-001")
        .build();

    // 测试序列化
    let json = serde_json::to_string(&event).map_err(|e| format!("审计事件序列化失败: {}", e))?;

    if json.is_empty() {
        return Err("序列化结果为空".to_string());
    }

    // 测试反序列化
    let _: AuditEvent =
        serde_json::from_str(&json).map_err(|e| format!("审计事件反序列化失败: {}", e))?;

    Ok(())
}

/// 测试消息追踪记录
pub fn test_message_tracing() -> Result<(), String> {
    // 创建消息追踪
    let mut trace = MessageTrace::new("MSG-001", "HIS", ProtocolType::Hl7V2);

    // 添加追踪跨度
    trace.add_span(TraceSpan {
        span_id: ulid::Ulid::new().to_string(),
        parent_span_id: None,
        operation: "receive".to_string(),
        component: "adapter".to_string(),
        start_time: chrono::Utc::now(),
        end_time: Some(chrono::Utc::now()),
        duration_ms: Some(5),
        status: SpanStatus::Ok,
        error: None,
        logs: Vec::new(),
        tags: std::collections::HashMap::new(),
    });

    trace.add_span(TraceSpan {
        span_id: ulid::Ulid::new().to_string(),
        parent_span_id: None,
        operation: "parse".to_string(),
        component: "hl7-adapter".to_string(),
        start_time: chrono::Utc::now(),
        end_time: Some(chrono::Utc::now()),
        duration_ms: Some(10),
        status: SpanStatus::Ok,
        error: None,
        logs: Vec::new(),
        tags: std::collections::HashMap::new(),
    });

    // 验证追踪跨度数量
    if trace.spans.len() != 2 {
        return Err("追踪跨度数量不正确".to_string());
    }

    // 完成追踪
    trace.complete(true);

    if trace.status != TraceStatus::Completed {
        return Err("追踪状态应该是 Completed".to_string());
    }

    Ok(())
}

/// 测试指标收集
pub async fn test_metrics_collection() -> Result<(), String> {
    let collector = MetricsCollector::new();

    // 记录计数器指标
    collector.increment("messages_received", 1).await;
    collector.increment("messages_received", 1).await;
    collector.increment("messages_received", 1).await;

    // 记录直方图指标
    collector.observe("message_processing_time_ms", 150.0).await;
    collector.observe("message_processing_time_ms", 200.0).await;
    collector.observe("message_processing_time_ms", 180.0).await;

    // 记录仪表指标
    collector.set_gauge("queue_size", 42).await;

    // 获取指标快照
    let metrics = collector.all_metrics().await;

    // 验证指标
    if let Some(count) = metrics.counters.get("messages_received") {
        if *count != 3 {
            return Err("消息接收计数不正确".to_string());
        }
    } else {
        return Err("找不到 messages_received 指标".to_string());
    }

    if let Some(gauge) = metrics.gauges.get("queue_size") {
        if *gauge != 42 {
            return Err("队列大小指标不正确".to_string());
        }
    } else {
        return Err("找不到 queue_size 指标".to_string());
    }

    Ok(())
}
