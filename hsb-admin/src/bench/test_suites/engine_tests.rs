//! 路由引擎测试

use hsb_common::{ProtocolType, SystemId};
use hsb_core::engine::{
    AdapterRegistry, DispatchResult, InMemoryRouter, ProcessingPipeline, Router,
};
use hsb_core::{
    DeliveryMode, Message, MessageBuilder, MessageContext, Route, RouteBuilder, RouteOptions,
    RouteTarget, SourceMatch,
};
use serde_json::json;

/// 创建测试消息
fn create_test_message() -> Message {
    MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .target_system(SystemId::new("LIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .payload(json!({"patient": "test", "event": "admission"}))
        .raw_payload(b"MSH|^~\\&|HIS|HOSPITAL|LIS|LAB|...")
        .build()
        .expect("创建测试消息失败")
}

/// 创建测试路由
fn create_test_route(id: &str, source: &str, target: &str) -> Route {
    RouteBuilder::new()
        .id(id)
        .name(&format!("{} to {} Route", source, target))
        .source(SourceMatch {
            system_id: Some(source.to_string()),
            protocol: Some(ProtocolType::Hl7V2),
            message_type_pattern: Some("ADT.*".to_string()),
        })
        .target(RouteTarget::primary(SystemId::new(target)))
        .options(RouteOptions {
            delivery_mode: DeliveryMode::AtLeastOnce,
            timeout_ms: 5000,
            async_delivery: true,
            require_ack: true,
            audit_enabled: true,
            dlq_on_failure: true,
        })
        .build()
        .expect("创建测试路由失败")
}

/// 测试路由器创建
pub async fn test_router_creation() -> Result<(), String> {
    let router = InMemoryRouter::new();

    // 验证初始状态
    let routes = router
        .list_routes()
        .await
        .map_err(|e| format!("列出路由失败: {:?}", e))?;

    if !routes.is_empty() {
        return Err("新创建的路由器应该没有路由".to_string());
    }

    Ok(())
}

/// 测试路由规则添加
pub async fn test_route_addition() -> Result<(), String> {
    let router = InMemoryRouter::new();

    // 添加多个路由
    let route1 = create_test_route("route-001", "HIS", "LIS");
    let route2 = create_test_route("route-002", "HIS", "RIS");
    let route3 = create_test_route("route-003", "HIS", "PACS");

    router
        .add_route(route1)
        .await
        .map_err(|e| format!("添加路由1失败: {:?}", e))?;
    router
        .add_route(route2)
        .await
        .map_err(|e| format!("添加路由2失败: {:?}", e))?;
    router
        .add_route(route3)
        .await
        .map_err(|e| format!("添加路由3失败: {:?}", e))?;

    // 验证路由数量
    let routes = router
        .list_routes()
        .await
        .map_err(|e| format!("列出路由失败: {:?}", e))?;

    if routes.len() != 3 {
        return Err(format!("路由数量应该是 3，实际是 {}", routes.len()));
    }

    Ok(())
}

/// 测试路由查找
pub async fn test_route_finding() -> Result<(), String> {
    let router = InMemoryRouter::new();

    // 添加路由
    let route = create_test_route("route-001", "HIS", "LIS");
    router
        .add_route(route)
        .await
        .map_err(|e| format!("添加路由失败: {:?}", e))?;

    // 创建测试消息
    let msg = create_test_message();

    // 查找匹配的路由
    let matches = router
        .find_routes(&msg)
        .await
        .map_err(|e| format!("查找路由失败: {:?}", e))?;

    if matches.is_empty() {
        return Err("应该找到匹配的路由".to_string());
    }

    Ok(())
}

/// 测试分发结果构建
pub fn test_dispatch_result() -> Result<(), String> {
    // 成功结果
    let success = DispatchResult::success("route-001", "LIS", Some(bytes::Bytes::from("ACK")), 150);

    if !success.success {
        return Err("成功结果的 success 应该为 true".to_string());
    }
    if success.duration_ms != 150 {
        return Err("耗时不匹配".to_string());
    }

    // 失败结果
    let failure = DispatchResult::failure("route-002", "RIS", "Connection timeout", 5000, 2);

    if failure.success {
        return Err("失败结果的 success 应该为 false".to_string());
    }
    if failure.retry_count != 2 {
        return Err("重试次数不匹配".to_string());
    }
    if failure.error.as_deref() != Some("Connection timeout") {
        return Err("错误信息不匹配".to_string());
    }

    Ok(())
}

/// 测试处理管道
pub async fn test_processing_pipeline() -> Result<(), String> {
    let _pipeline = ProcessingPipeline::new();

    // 验证管道创建成功
    // ProcessingPipeline::new() 不需要名称参数

    // 创建消息上下文
    let msg = create_test_message();
    let ctx = MessageContext::new(msg);

    // 验证上下文 - message() 是 async 方法
    let msg = ctx.message().await;
    if msg.source_system.to_string() != "HIS" {
        return Err("消息源系统不匹配".to_string());
    }

    Ok(())
}

/// 测试适配器注册表
pub fn test_adapter_registry() -> Result<(), String> {
    let registry = AdapterRegistry::new();

    // 验证初始状态：没有注册任何适配器
    let protocols = registry.supported_protocols();

    // 注册表创建成功，初始为空
    if !protocols.is_empty() {
        return Err("新建注册表应该为空".to_string());
    }

    Ok(())
}
