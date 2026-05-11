//! 核心模块测试

use hsb_common::{MessagePriority, ProtocolType, SystemId};
use hsb_core::{
    Message, MessageBuilder, MessageContext, MessageMetadata, RouteBuilder, RouteTarget,
    SourceMatch, TransformerChain,
};
use serde_json::json;

/// 测试消息创建
pub fn test_message_creation() -> Result<(), String> {
    let msg = MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .target_system(SystemId::new("LIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .payload(json!({
            "patient_id": "P12345",
            "event": "admission"
        }))
        .raw_payload(b"MSH|^~\\&|HIS|HOSPITAL|LIS|LAB|...")
        .priority(MessagePriority::High)
        .build()
        .map_err(|e| format!("消息创建失败: {:?}", e))?;

    if msg.source_system.to_string() != "HIS" {
        return Err("源系统不匹配".to_string());
    }

    Ok(())
}

/// 测试消息序列化
pub fn test_message_serialization() -> Result<(), String> {
    let msg = MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .payload(json!({"patient": "test"}))
        .raw_payload(b"test message")
        .build()
        .map_err(|e| format!("消息创建失败: {:?}", e))?;

    let json = serde_json::to_string(&msg).map_err(|e| format!("序列化失败: {}", e))?;

    if json.is_empty() {
        return Err("序列化结果为空".to_string());
    }

    Ok(())
}

/// 测试消息反序列化
pub fn test_message_deserialization() -> Result<(), String> {
    let msg = MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .payload(json!({"patient": "test"}))
        .raw_payload(b"test message")
        .build()
        .map_err(|e| format!("消息创建失败: {:?}", e))?;

    let json = serde_json::to_string(&msg).map_err(|e| format!("序列化失败: {}", e))?;

    let _: Message = serde_json::from_str(&json).map_err(|e| format!("反序列化失败: {}", e))?;

    Ok(())
}

/// 测试消息元数据操作
pub fn test_metadata_operations() -> Result<(), String> {
    let mut metadata = MessageMetadata::default();
    metadata.patient_id = Some("P12345".to_string());
    metadata.visit_id = Some("V67890".to_string());
    metadata.department_code = Some("INTERNAL".to_string());
    metadata.sending_application = Some("HIS".to_string());
    metadata.sending_facility = Some("HOSPITAL".to_string());

    if metadata.patient_id.as_deref() != Some("P12345") {
        return Err("患者ID不匹配".to_string());
    }

    // 测试序列化
    let json = serde_json::to_string(&metadata).map_err(|e| format!("元数据序列化失败: {}", e))?;

    if json.is_empty() {
        return Err("元数据序列化为空".to_string());
    }

    Ok(())
}

/// 测试路由规则创建
pub fn test_route_creation() -> Result<(), String> {
    let route = RouteBuilder::new()
        .id("route-001")
        .name("HIS to LIS Route")
        .source(SourceMatch::system("HIS"))
        .target(RouteTarget::primary(SystemId::new("LIS")))
        .build()
        .map_err(|e| format!("路由创建失败: {:?}", e))?;

    if route.id.to_string() != "route-001" {
        return Err("路由ID不匹配".to_string());
    }

    Ok(())
}

/// 测试路由匹配
pub fn test_route_matching() -> Result<(), String> {
    // 创建路由
    let route = RouteBuilder::new()
        .id("route-001")
        .name("Test Route")
        .source(SourceMatch {
            system_id: Some("HIS".to_string()),
            protocol: Some(ProtocolType::Hl7V2),
            message_type_pattern: Some("ADT.*".to_string()),
        })
        .target(RouteTarget::primary(SystemId::new("LIS")))
        .build()
        .map_err(|e| format!("路由创建失败: {:?}", e))?;

    // 创建测试消息
    let msg = MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .raw_payload(b"test")
        .build()
        .map_err(|e| format!("消息创建失败: {:?}", e))?;

    // 验证匹配
    if !route.matches(&msg) {
        return Err("消息应该匹配路由".to_string());
    }

    Ok(())
}

/// 测试转换器链
pub fn test_transformer_chain() -> Result<(), String> {
    let chain = TransformerChain::new();

    // 验证链创建成功
    if !chain.is_empty() {
        return Err("新建转换器链应该为空".to_string());
    }

    if chain.len() != 0 {
        return Err("新建转换器链长度应该为0".to_string());
    }

    Ok(())
}

/// 测试消息上下文
pub async fn test_message_context() -> Result<(), String> {
    let msg = MessageBuilder::new()
        .source_system(SystemId::new("HIS"))
        .protocol(ProtocolType::Hl7V2)
        .message_type("ADT^A01")
        .raw_payload(b"test")
        .build()
        .map_err(|e| format!("消息创建失败: {:?}", e))?;

    let ctx = MessageContext::new(msg);
    ctx.set_attribute("test_key", "test_value").await;

    let value = ctx.get_attribute("test_key").await;
    if value.is_none() {
        return Err("上下文属性未找到".to_string());
    }

    Ok(())
}
