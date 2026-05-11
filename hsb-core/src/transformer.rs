//! 消息转换器定义
//!
//! 支持消息格式转换（如 HL7 ↔ FHIR）和字段映射。

use crate::message::Message;
use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// 转换上下文
#[derive(Debug, Clone, Default)]
pub struct TransformContext {
    /// 上下文变量
    pub variables: HashMap<String, serde_json::Value>,

    /// 是否为调试模式
    pub debug: bool,

    /// 转换链中的位置
    pub chain_index: usize,
}

impl TransformContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_variable(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.variables.insert(key.into(), value);
        self
    }

    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.variables.insert(key.into(), value);
    }
}

/// 转换器 trait
///
/// 所有消息转换器都实现此 trait。
#[async_trait]
pub trait Transformer: Send + Sync {
    /// 转换器 ID
    fn id(&self) -> &str;

    /// 转换器名称
    fn name(&self) -> &str;

    /// 执行转换
    async fn transform(&self, msg: Message, ctx: &mut TransformContext) -> HsbResult<Message>;

    /// 验证转换器配置
    fn validate(&self) -> HsbResult<()> {
        Ok(())
    }
}

/// 转换器链
///
/// 按顺序执行多个转换器。
pub struct TransformerChain {
    transformers: Vec<Arc<dyn Transformer>>,
}

impl TransformerChain {
    pub fn new() -> Self {
        Self {
            transformers: Vec::new(),
        }
    }

    pub fn add(mut self, transformer: Arc<dyn Transformer>) -> Self {
        self.transformers.push(transformer);
        self
    }

    pub async fn execute(&self, mut msg: Message) -> HsbResult<Message> {
        let mut ctx = TransformContext::new();

        for (index, transformer) in self.transformers.iter().enumerate() {
            ctx.chain_index = index;
            tracing::debug!(
                transformer_id = transformer.id(),
                transformer_name = transformer.name(),
                chain_index = index,
                "Executing transformer"
            );

            msg = transformer.transform(msg, &mut ctx).await?;
        }

        Ok(msg)
    }

    pub fn len(&self) -> usize {
        self.transformers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transformers.is_empty()
    }
}

impl Default for TransformerChain {
    fn default() -> Self {
        Self::new()
    }
}

// ============ 内置转换器 ============

/// 字段映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// 源字段路径
    pub source: String,
    /// 目标字段路径
    pub target: String,
    /// 是否必需
    pub required: bool,
    /// 默认值
    pub default_value: Option<serde_json::Value>,
    /// 值转换表达式
    pub transform_expr: Option<String>,
}

/// 字段映射转换器
pub struct FieldMappingTransformer {
    id: String,
    name: String,
    mappings: Vec<FieldMapping>,
}

impl FieldMappingTransformer {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            mappings: Vec::new(),
        }
    }

    pub fn with_mapping(mut self, mapping: FieldMapping) -> Self {
        self.mappings.push(mapping);
        self
    }

    pub fn with_mappings(mut self, mappings: Vec<FieldMapping>) -> Self {
        self.mappings.extend(mappings);
        self
    }
}

#[async_trait]
impl Transformer for FieldMappingTransformer {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn transform(&self, mut msg: Message, _ctx: &mut TransformContext) -> HsbResult<Message> {
        let mut output = serde_json::Map::new();

        for mapping in &self.mappings {
            let value = msg
                .get_payload_field(&mapping.source)
                .cloned()
                .or_else(|| mapping.default_value.clone());

            match value {
                Some(v) => {
                    // 简单的路径设置（支持嵌套）
                    set_nested_value(&mut output, &mapping.target, v);
                }
                None if mapping.required => {
                    return Err(HsbError::FieldMappingError {
                        source_field: mapping.source.clone(),
                        target_field: mapping.target.clone(),
                        reason: "Required field is missing".to_string(),
                    });
                }
                None => {}
            }
        }

        msg.payload = Some(serde_json::Value::Object(output));
        Ok(msg)
    }
}

/// 设置嵌套 JSON 值
fn set_nested_value(
    map: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = map;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            current.insert(part.to_string(), value.clone());
        } else {
            current = current
                .entry(part.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                .as_object_mut()
                .unwrap_or_else(|| panic!("Path conflict at {}", part));
        }
    }
}

/// Header 操作转换器
pub struct HeaderTransformer {
    id: String,
    name: String,
    operations: Vec<HeaderOperation>,
}

/// Header 操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeaderOperation {
    Set { key: String, value: String },
    Remove { key: String },
    Rename { from: String, to: String },
    Copy { from: String, to: String },
}

impl HeaderTransformer {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            operations: Vec::new(),
        }
    }

    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.operations.push(HeaderOperation::Set {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.operations
            .push(HeaderOperation::Remove { key: key.into() });
        self
    }
}

#[async_trait]
impl Transformer for HeaderTransformer {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn transform(&self, mut msg: Message, _ctx: &mut TransformContext) -> HsbResult<Message> {
        for op in &self.operations {
            match op {
                HeaderOperation::Set { key, value } => {
                    msg.headers.insert(key.clone(), value.clone());
                }
                HeaderOperation::Remove { key } => {
                    msg.headers.remove(key);
                }
                HeaderOperation::Rename { from, to } => {
                    if let Some(value) = msg.headers.remove(from) {
                        msg.headers.insert(to.clone(), value);
                    }
                }
                HeaderOperation::Copy { from, to } => {
                    if let Some(value) = msg.headers.get(from).cloned() {
                        msg.headers.insert(to.clone(), value);
                    }
                }
            }
        }
        Ok(msg)
    }
}

/// 空转换器（透传）
pub struct PassthroughTransformer {
    id: String,
}

impl PassthroughTransformer {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Transformer for PassthroughTransformer {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Passthrough"
    }

    async fn transform(&self, msg: Message, _ctx: &mut TransformContext) -> HsbResult<Message> {
        Ok(msg)
    }
}

/// JSON 转 XML 转换器（占位）
#[allow(dead_code)]
pub struct JsonToXmlTransformer {
    id: String,
    root_element: String,
}

impl JsonToXmlTransformer {
    pub fn new(id: impl Into<String>, root_element: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            root_element: root_element.into(),
        }
    }
}

#[async_trait]
impl Transformer for JsonToXmlTransformer {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "JSON to XML"
    }

    async fn transform(&self, msg: Message, _ctx: &mut TransformContext) -> HsbResult<Message> {
        // TODO: 实现 JSON 到 XML 的转换
        tracing::warn!("JsonToXmlTransformer not fully implemented");
        Ok(msg)
    }
}

/// XML 转 JSON 转换器（占位）
pub struct XmlToJsonTransformer {
    id: String,
}

impl XmlToJsonTransformer {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Transformer for XmlToJsonTransformer {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "XML to JSON"
    }

    async fn transform(&self, msg: Message, _ctx: &mut TransformContext) -> HsbResult<Message> {
        // TODO: 实现 XML 到 JSON 的转换
        tracing::warn!("XmlToJsonTransformer not fully implemented");
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageBuilder;
    use hsb_common::ProtocolType;

    #[tokio::test]
    async fn test_header_transformer() {
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .header("Content-Type", "application/json")
            .raw_payload("{}")
            .build()
            .expect("Message should be valid");

        let transformer = HeaderTransformer::new("t1", "Test Header Transform")
            .set("X-Custom", "value")
            .remove("Content-Type");

        let mut ctx = TransformContext::new();
        let result = transformer
            .transform(msg, &mut ctx)
            .await
            .expect("Transform should succeed");

        assert!(result.get_header("X-Custom").is_some());
        assert!(result.get_header("Content-Type").is_none());
    }

    #[tokio::test]
    async fn test_transformer_chain() {
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .raw_payload("{}")
            .build()
            .expect("Message should be valid");

        let chain = TransformerChain::new()
            .add(Arc::new(
                HeaderTransformer::new("t1", "Add Header").set("X-Step", "1"),
            ))
            .add(Arc::new(
                HeaderTransformer::new("t2", "Add Another").set("X-Step2", "2"),
            ));

        let result = chain.execute(msg).await.expect("Chain should succeed");

        assert_eq!(result.get_header("X-Step"), Some(&"1".to_string()));
        assert_eq!(result.get_header("X-Step2"), Some(&"2".to_string()));
    }
}
