//! HSB HL7 v2.x 协议适配器
//!
//! 支持 HL7 v2.x 消息的解析和生成。

mod ack;
mod parser;
mod segment;
mod serializer;
mod types;

pub use ack::*;
pub use parser::Hl7Parser;
pub use segment::*;
pub use serializer::Hl7Serializer;
pub use types::*;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, ProtocolType};
use hsb_core::Message;
use hsb_core::{ParseOptions, ProtocolAdapter, SerializeOptions, ValidationResult};

/// HL7 v2.x 适配器
pub struct Hl7V2Adapter {
    parser: Hl7Parser,
    serializer: Hl7Serializer,
}

impl Hl7V2Adapter {
    pub fn new() -> Self {
        Self {
            parser: Hl7Parser::new(),
            serializer: Hl7Serializer::new(),
        }
    }

    /// 创建带配置的适配器
    pub fn with_config(config: Hl7Config) -> Self {
        Self {
            parser: Hl7Parser::with_config(config.clone()),
            serializer: Hl7Serializer::with_config(config),
        }
    }
}

impl Default for Hl7V2Adapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAdapter for Hl7V2Adapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Hl7V2
    }

    fn name(&self) -> &str {
        "HL7 v2.x Adapter"
    }

    async fn parse(&self, raw: Bytes, options: &ParseOptions) -> HsbResult<Message> {
        self.parser.parse(raw, options)
    }

    async fn serialize(&self, msg: &Message, options: &SerializeOptions) -> HsbResult<Bytes> {
        self.serializer.serialize(msg, options)
    }

    async fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        self.parser.validate(raw)
    }

    fn extract_message_type(&self, raw: &Bytes) -> Option<String> {
        self.parser.extract_message_type(raw)
    }
}

/// HL7 配置
#[derive(Debug, Clone)]
pub struct Hl7Config {
    /// HL7 版本
    pub version: Hl7Version,
    /// 是否严格验证
    pub strict_validation: bool,
    /// 默认编码字符
    pub encoding_characters: String,
    /// 是否允许空段
    pub allow_empty_segments: bool,
    /// 时间戳格式
    pub timestamp_format: String,
}

impl Default for Hl7Config {
    fn default() -> Self {
        Self {
            version: Hl7Version::V2_5_1,
            strict_validation: false,
            encoding_characters: "^~\\&".to_string(),
            allow_empty_segments: true,
            timestamp_format: "%Y%m%d%H%M%S".to_string(),
        }
    }
}

/// HL7 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hl7Version {
    V2_1,
    V2_2,
    V2_3,
    V2_3_1,
    V2_4,
    V2_5,
    V2_5_1,
    V2_6,
    V2_7,
    V2_8,
    V2_9,
    V2_10,
}

impl Hl7Version {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V2_1 => "2.1",
            Self::V2_2 => "2.2",
            Self::V2_3 => "2.3",
            Self::V2_3_1 => "2.3.1",
            Self::V2_4 => "2.4",
            Self::V2_5 => "2.5",
            Self::V2_5_1 => "2.5.1",
            Self::V2_6 => "2.6",
            Self::V2_7 => "2.7",
            Self::V2_8 => "2.8",
            Self::V2_9 => "2.9",
            Self::V2_10 => "2.10",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = Hl7V2Adapter::new();
        assert_eq!(adapter.protocol(), ProtocolType::Hl7V2);
        assert_eq!(adapter.name(), "HL7 v2.x Adapter");
    }
}
