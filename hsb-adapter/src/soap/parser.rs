//! SOAP 消息解析器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use hsb_core::{ErrorSeverity, ParseOptions, ValidationError, ValidationResult};
use hsb_core::{Message, MessageBuilder};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};

use super::SoapConfig;

/// SOAP 解析器
#[allow(dead_code)]
pub struct SoapParser {
    config: SoapConfig,
}

impl SoapParser {
    pub fn new() -> Self {
        Self {
            config: SoapConfig::default(),
        }
    }

    pub fn with_config(config: SoapConfig) -> Self {
        Self { config }
    }

    /// 解析 SOAP 消息
    pub fn parse(&self, raw: Bytes, _options: &ParseOptions) -> HsbResult<Message> {
        let content = String::from_utf8(raw.to_vec()).map_err(|e| HsbError::ParseError {
            message: format!("Invalid UTF-8: {}", e),
        })?;

        // 解析 XML
        let mut reader = Reader::from_str(&content);
        reader.config_mut().trim_text(true);

        let mut payload = Map::new();
        let mut in_body = false;
        let mut body_content = String::new();
        let mut action: Option<String> = None;
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if name.ends_with(":Body") || name == "Body" {
                        in_body = true;
                    } else if in_body && action.is_none() {
                        // 第一个 Body 内的元素通常是 Action
                        action = Some(name.clone());
                    }

                    // 提取 WS-Addressing Action
                    if name.ends_with(":Action") || name == "Action" {
                        // 下一个文本事件是 Action 值
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.ends_with(":Body") || name == "Body" {
                        in_body = false;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_body {
                        body_content.push_str(&e.unescape().unwrap_or_default());
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(HsbError::ParseError {
                        message: format!("XML parse error: {}", e),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        payload.insert("body".to_string(), json!(body_content));

        // 构建消息
        let mut builder = MessageBuilder::new()
            .source_system("SOAP")
            .protocol(ProtocolType::Soap)
            .payload(Value::Object(payload))
            .raw_payload(raw);

        if let Some(act) = action {
            builder = builder.message_type(act);
        }

        builder.build()
    }

    /// 验证 SOAP 消息
    pub fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        let content = match String::from_utf8(raw.to_vec()) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ValidationResult::invalid(vec![ValidationError {
                    code: "SOAP-001".to_string(),
                    message: format!("Invalid UTF-8: {}", e),
                    location: None,
                    severity: ErrorSeverity::Fatal,
                }]));
            }
        };

        let mut reader = Reader::from_str(&content);
        let mut buf = Vec::new();
        let mut has_envelope = false;
        let mut has_body = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.ends_with(":Envelope") || name == "Envelope" {
                        has_envelope = true;
                    }
                    if name.ends_with(":Body") || name == "Body" {
                        has_body = true;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Ok(ValidationResult::invalid(vec![ValidationError {
                        code: "SOAP-002".to_string(),
                        message: format!("XML parse error: {}", e),
                        location: None,
                        severity: ErrorSeverity::Fatal,
                    }]));
                }
                _ => {}
            }
            buf.clear();
        }

        let mut errors = Vec::new();

        if !has_envelope {
            errors.push(ValidationError {
                code: "SOAP-003".to_string(),
                message: "Missing SOAP Envelope".to_string(),
                location: None,
                severity: ErrorSeverity::Error,
            });
        }

        if !has_body {
            errors.push(ValidationError {
                code: "SOAP-004".to_string(),
                message: "Missing SOAP Body".to_string(),
                location: None,
                severity: ErrorSeverity::Error,
            });
        }

        if errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(errors))
        }
    }

    /// 提取 SOAP Action
    pub fn extract_action(&self, raw: &Bytes) -> Option<String> {
        let content = String::from_utf8(raw.to_vec()).ok()?;
        let mut reader = Reader::from_str(&content);
        let mut buf = Vec::new();
        let mut in_body = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.ends_with(":Body") || name == "Body" {
                        in_body = true;
                    } else if in_body {
                        // 第一个 Body 子元素
                        return Some(name);
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        None
    }
}

impl Default for SoapParser {
    fn default() -> Self {
        Self::new()
    }
}
