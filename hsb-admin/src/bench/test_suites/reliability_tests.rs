//! 可靠性层测试

use std::time::Duration;

use hsb_common::{ProtocolType, SystemId};
use hsb_core::reliability::{
    CircuitBreaker, CircuitBreakerConfig, CircuitState, DeadLetter, DeadLetterFilter,
    DeadLetterQueue, DeadLetterReason, InMemoryDlq, InMemoryQueue, MessageMeta, MessageStatus,
    RetryStrategy,
};
use hsb_core::{Message, MessageBuilder};
use serde_json::json;

/// 创建测试消息
fn create_test_message() -> Message {
    MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .payload(json!({"patient": "test"}))
        .raw_payload(b"test message")
        .build()
        .expect("创建测试消息失败")
}

/// 测试队列入队
pub async fn test_queue_enqueue() -> Result<(), String> {
    let queue = InMemoryQueue::new(1000);
    let msg = create_test_message();

    queue
        .enqueue(msg)
        .await
        .map_err(|e| format!("入队失败: {:?}", e))?;

    // 验证队列大小
    if queue.len().await != 1 {
        return Err("队列大小不正确".to_string());
    }

    Ok(())
}

/// 测试队列出队
pub async fn test_queue_dequeue() -> Result<(), String> {
    let queue = InMemoryQueue::new(1000);
    let msg = create_test_message();
    let msg_id = msg.id;

    queue
        .enqueue(msg)
        .await
        .map_err(|e| format!("入队失败: {:?}", e))?;

    let dequeued = queue
        .dequeue()
        .await
        .map_err(|e| format!("出队失败: {:?}", e))?;

    match dequeued {
        Some(m) => {
            if m.message.id != msg_id {
                return Err("消息 ID 不匹配".to_string());
            }
        }
        None => return Err("队列应该不为空".to_string()),
    }

    Ok(())
}

/// 测试重试策略计算
pub fn test_retry_strategy() -> Result<(), String> {
    // 固定延迟策略
    let fixed = RetryStrategy::Fixed { delay_ms: 1000 };
    let delay = fixed.delay_for_attempt(1);
    if delay != Duration::from_millis(1000) {
        return Err("固定延迟计算错误".to_string());
    }

    // 指数退避策略
    let exponential = RetryStrategy::Exponential {
        base_delay_ms: 100,
        multiplier: 2.0,
        max_delay_ms: 10000,
    };

    let delay1 = exponential.delay_for_attempt(1);
    let delay2 = exponential.delay_for_attempt(2);
    let delay3 = exponential.delay_for_attempt(3);

    // 验证指数增长
    if delay2 <= delay1 {
        return Err("指数退避延迟应该递增".to_string());
    }
    if delay3 <= delay2 {
        return Err("指数退避延迟应该递增".to_string());
    }

    // 验证最大延迟限制
    let delay10 = exponential.delay_for_attempt(10);
    if delay10 > Duration::from_millis(10000) {
        return Err("延迟超过最大限制".to_string());
    }

    Ok(())
}

/// 测试熔断器状态转换
pub async fn test_circuit_breaker() -> Result<(), String> {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        open_duration_secs: 1,
        failure_window_secs: 60,
    };

    let breaker = CircuitBreaker::new("test-breaker", config);

    // 初始状态应该是关闭
    if breaker.state().await != CircuitState::Closed {
        return Err("初始状态应该是 Closed".to_string());
    }

    // 记录成功
    breaker.record_success().await;

    // 仍然应该是关闭状态
    if breaker.state().await != CircuitState::Closed {
        return Err("成功后状态应该仍是 Closed".to_string());
    }

    // 记录多次失败
    for _ in 0..3 {
        breaker.record_failure().await;
    }

    // 应该转为开启状态
    if breaker.state().await != CircuitState::Open {
        return Err("多次失败后应该是 Open 状态".to_string());
    }

    Ok(())
}

/// 测试死信队列操作
pub async fn test_dlq_operations() -> Result<(), String> {
    let dlq = InMemoryDlq::new(1000);
    let msg = create_test_message();

    // 创建死信
    let dead_letter = DeadLetter {
        message: msg,
        reason: DeadLetterReason::MaxRetriesExceeded,
        error_detail: "测试失败：超过最大重试次数".to_string(),
        retry_count: 3,
        last_processed_at: chrono::Utc::now(),
        dead_lettered_at: chrono::Utc::now(),
        source_route_id: Some("route-001".to_string()),
        target_system: Some("LIS".to_string()),
    };

    // 添加到死信队列
    dlq.add(dead_letter)
        .await
        .map_err(|e| format!("添加死信失败: {:?}", e))?;

    // 查询死信
    let filter = DeadLetterFilter::default();

    let letters = dlq
        .list(filter)
        .await
        .map_err(|e| format!("查询死信失败: {:?}", e))?;

    if letters.is_empty() {
        return Err("死信队列应该不为空".to_string());
    }

    Ok(())
}

/// 测试消息状态追踪
pub fn test_message_status() -> Result<(), String> {
    let msg = create_test_message();
    let mut meta = MessageMeta::new(&msg);

    // 验证初始状态
    if meta.status != MessageStatus::Pending {
        return Err("初始状态应该是 Pending".to_string());
    }

    // 更新状态
    meta.status = MessageStatus::Processing;
    if meta.status != MessageStatus::Processing {
        return Err("状态更新失败".to_string());
    }

    // 增加重试次数
    meta.retry_count += 1;
    meta.last_error = Some("临时错误".to_string());
    meta.status = MessageStatus::Retrying;

    if meta.retry_count != 1 {
        return Err("重试次数不正确".to_string());
    }

    // 完成处理
    meta.status = MessageStatus::Completed;
    if meta.status != MessageStatus::Completed {
        return Err("最终状态应该是 Completed".to_string());
    }

    Ok(())
}
