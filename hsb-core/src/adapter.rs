//! HSB Adapter Base - 协议适配器基础 trait
//!
//! 定义所有协议适配器必须实现的接口。

use crate::Message;
use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, ProtocolType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 协议适配器 trait
///
/// 所有协议适配器（HL7、FHIR、DICOM 等）都必须实现此 trait。
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// 获取适配器支持的协议类型
    fn protocol(&self) -> ProtocolType;

    /// 适配器名称
    fn name(&self) -> &str;

    /// 解析原始报文为 Message
    async fn parse(&self, raw: Bytes, options: &ParseOptions) -> HsbResult<Message>;

    /// 将 Message 序列化为目标协议格式
    async fn serialize(&self, msg: &Message, options: &SerializeOptions) -> HsbResult<Bytes>;

    /// 验证原始报文格式
    async fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult>;

    /// 提取消息类型（不完全解析）
    fn extract_message_type(&self, raw: &Bytes) -> Option<String>;

    /// 是否支持流式处理
    fn supports_streaming(&self) -> bool {
        false
    }
}

/// 解析选项
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParseOptions {
    /// 是否严格模式（严格验证）
    pub strict_mode: bool,
    /// 字符编码
    pub encoding: Option<String>,
    /// 是否保留原始报文
    pub preserve_raw: bool,
    /// 自定义选项
    pub custom: HashMap<String, String>,
}

impl ParseOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn strict() -> Self {
        Self {
            strict_mode: true,
            ..Default::default()
        }
    }

    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = Some(encoding.into());
        self
    }
}

/// 序列化选项
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SerializeOptions {
    /// 是否美化输出
    pub pretty_print: bool,
    /// 字符编码
    pub encoding: Option<String>,
    /// 是否包含 XML 声明
    pub include_xml_declaration: bool,
    /// 自定义选项
    pub custom: HashMap<String, String>,
}

impl SerializeOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pretty() -> Self {
        Self {
            pretty_print: true,
            ..Default::default()
        }
    }
}

/// 验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// 是否有效
    pub valid: bool,
    /// 错误列表
    pub errors: Vec<ValidationError>,
    /// 警告列表
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: ValidationWarning) -> Self {
        self.warnings.push(warning);
        self
    }
}

/// 验证错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// 错误代码
    pub code: String,
    /// 错误消息
    pub message: String,
    /// 错误位置（如行号、字段路径）
    pub location: Option<String>,
    /// 错误严重性
    pub severity: ErrorSeverity,
}

/// 验证警告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// 警告代码
    pub code: String,
    /// 警告消息
    pub message: String,
    /// 警告位置
    pub location: Option<String>,
}

/// 错误严重性
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorSeverity {
    /// 致命错误（无法继续）
    Fatal,
    /// 错误（可能继续但有问题）
    Error,
    /// 警告
    Warning,
    /// 信息
    Info,
}

/// 适配器工厂 trait
pub trait AdapterFactory: Send + Sync {
    /// 创建适配器实例
    fn create(&self) -> Box<dyn ProtocolAdapter>;

    /// 支持的协议类型
    fn supported_protocol(&self) -> ProtocolType;
}

/// 适配器注册表
pub struct AdapterRegistry {
    adapters: HashMap<ProtocolType, Box<dyn ProtocolAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// 注册适配器
    pub fn register(&mut self, adapter: Box<dyn ProtocolAdapter>) {
        let protocol = adapter.protocol();
        tracing::info!(protocol = ?protocol, name = adapter.name(), "Registering protocol adapter");
        self.adapters.insert(protocol, adapter);
    }

    /// 获取适配器
    pub fn get(&self, protocol: ProtocolType) -> Option<&dyn ProtocolAdapter> {
        self.adapters.get(&protocol).map(|a| a.as_ref())
    }

    /// 获取所有支持的协议
    pub fn supported_protocols(&self) -> Vec<ProtocolType> {
        self.adapters.keys().copied().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result() {
        let result = ValidationResult::valid();
        assert!(result.valid);
        assert!(result.errors.is_empty());

        let error = ValidationError {
            code: "E001".to_string(),
            message: "Invalid segment".to_string(),
            location: Some("MSH".to_string()),
            severity: ErrorSeverity::Error,
        };
        let result = ValidationResult::invalid(vec![error]);
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
    }
}
