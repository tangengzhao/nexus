//! HSB SOAP/WebService 协议适配器
//!
//! 支持 SOAP 1.1/1.2 消息的解析和生成。

mod envelope;
mod parser;
mod serializer;

pub use envelope::*;
pub use parser::SoapParser;
pub use serializer::SoapSerializer;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, ProtocolType};
use hsb_core::Message;
use hsb_core::{ParseOptions, ProtocolAdapter, SerializeOptions, ValidationResult};

/// SOAP 适配器
#[allow(dead_code)]
pub struct SoapAdapter {
    parser: SoapParser,
    serializer: SoapSerializer,
    config: SoapConfig,
}

impl SoapAdapter {
    pub fn new() -> Self {
        Self {
            parser: SoapParser::new(),
            serializer: SoapSerializer::new(),
            config: SoapConfig::default(),
        }
    }

    pub fn with_config(config: SoapConfig) -> Self {
        Self {
            parser: SoapParser::with_config(config.clone()),
            serializer: SoapSerializer::with_config(config.clone()),
            config,
        }
    }
}

impl Default for SoapAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAdapter for SoapAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Soap
    }

    fn name(&self) -> &str {
        "SOAP/WebService Adapter"
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
        self.parser.extract_action(raw)
    }
}

/// SOAP 配置
#[derive(Debug, Clone)]
pub struct SoapConfig {
    /// SOAP 版本
    pub version: SoapVersion,
    /// 默认命名空间
    pub default_namespace: Option<String>,
    /// WS-Security 配置
    pub ws_security: Option<WsSecurityConfig>,
    /// 是否验证 Schema
    pub validate_schema: bool,
}

impl Default for SoapConfig {
    fn default() -> Self {
        Self {
            version: SoapVersion::Soap12,
            default_namespace: None,
            ws_security: None,
            validate_schema: false,
        }
    }
}

/// SOAP 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoapVersion {
    Soap11,
    Soap12,
}

impl SoapVersion {
    pub fn namespace(&self) -> &'static str {
        match self {
            Self::Soap11 => "http://schemas.xmlsoap.org/soap/envelope/",
            Self::Soap12 => "http://www.w3.org/2003/05/soap-envelope",
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Soap11 => "text/xml; charset=utf-8",
            Self::Soap12 => "application/soap+xml; charset=utf-8",
        }
    }
}

/// WS-Security 配置
#[derive(Debug, Clone)]
pub struct WsSecurityConfig {
    /// 用户名
    pub username: Option<String>,
    /// 密码
    pub password: Option<String>,
    /// 密码类型
    pub password_type: WsPasswordType,
    /// 是否添加时间戳
    pub add_timestamp: bool,
    /// 是否添加 Nonce
    pub add_nonce: bool,
}

/// WS-Security 密码类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsPasswordType {
    /// 明文
    PasswordText,
    /// 摘要
    PasswordDigest,
}

impl WsPasswordType {
    pub fn type_uri(&self) -> &'static str {
        match self {
            Self::PasswordText => {
                "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordText"
            }
            Self::PasswordDigest => {
                "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest"
            }
        }
    }
}

/// SOAP 命名空间
pub mod namespace {
    pub const SOAP_11_ENVELOPE: &str = "http://schemas.xmlsoap.org/soap/envelope/";
    pub const SOAP_12_ENVELOPE: &str = "http://www.w3.org/2003/05/soap-envelope";
    pub const WSSE: &str =
        "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd";
    pub const WSU: &str =
        "http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd";
    pub const WSA: &str = "http://www.w3.org/2005/08/addressing";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = SoapAdapter::new();
        assert_eq!(adapter.protocol(), ProtocolType::Soap);
        assert_eq!(adapter.name(), "SOAP/WebService Adapter");
    }
}
