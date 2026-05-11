//! HSB HL7 v3 协议适配器
//!
//! 当前实现聚焦 HL7 v3 XML 报文的基本识别、验证与内部 Message 映射。

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use hsb_core::{
    ErrorSeverity, Message, MessageBuilder, ParseOptions, ProtocolAdapter, SerializeOptions,
    ValidationError, ValidationResult,
};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value};

pub struct Hl7V3Adapter;

impl Hl7V3Adapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Hl7V3Adapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAdapter for Hl7V3Adapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Hl7V3
    }

    fn name(&self) -> &str {
        "HL7 v3 Adapter"
    }

    async fn parse(&self, raw: Bytes, _options: &ParseOptions) -> HsbResult<Message> {
        let content = String::from_utf8(raw.to_vec()).map_err(|error| HsbError::ParseError {
            message: format!("Invalid UTF-8: {}", error),
        })?;

        let parsed = parse_hl7v3_xml(&content)?;
        let mut payload = Map::new();
        payload.insert(
            "root_element".to_string(),
            Value::String(parsed.root_element.clone()),
        );
        payload.insert(
            "namespace".to_string(),
            Value::String(parsed.namespace.clone()),
        );
        payload.insert("raw_xml".to_string(), Value::String(content.clone()));
        if let Some(interaction_id) = parsed.interaction_id.clone() {
            payload.insert("interaction_id".to_string(), Value::String(interaction_id));
        }
        if let Some(message_id) = parsed.message_id.clone() {
            payload.insert("message_id".to_string(), Value::String(message_id));
        }
        if let Some(patient_id) = parsed.patient_id.clone() {
            payload.insert("patient_id".to_string(), Value::String(patient_id));
        }

        let mut builder = MessageBuilder::new()
            .source_system("HL7V3")
            .protocol(ProtocolType::Hl7V3)
            .payload(Value::Object(payload))
            .raw_payload(raw);

        if let Some(message_type) = parsed.interaction_id.or(Some(parsed.root_element)) {
            builder = builder.message_type(message_type);
        }

        if let Some(patient_id) = parsed.patient_id {
            builder = builder.patient_id(patient_id);
        }

        builder.build()
    }

    async fn serialize(&self, msg: &Message, options: &SerializeOptions) -> HsbResult<Bytes> {
        if let Some(payload) = msg.payload.as_ref() {
            if let Some(raw_xml) = payload.get("raw_xml").and_then(Value::as_str) {
                return Ok(Bytes::from(raw_xml.to_string()));
            }
        }

        if !msg.raw_payload.is_empty() {
            return Ok(Bytes::from(msg.raw_payload.clone()));
        }

        let root = msg.message_type.as_deref().unwrap_or("PRPA_IN201301UV02");
        let patient_id = msg.metadata.patient_id.as_deref().unwrap_or("UNKNOWN");
        let newline = if options.pretty_print { "\n" } else { "" };
        let indent = if options.pretty_print { "  " } else { "" };
        let declaration = if options.include_xml_declaration {
            format!(r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}"#, newline)
        } else {
            String::new()
        };

        Ok(Bytes::from(format!(
            concat!(
                "{declaration}",
                "<{root} xmlns=\"urn:hl7-org:v3\">{newline}",
                "{indent}<id extension=\"{message_id}\" root=\"2.16.156.10011.2.2.1\"/>{newline}",
                "{indent}<interactionId extension=\"{root}\" root=\"2.16.840.1.113883.1.6\"/>{newline}",
                "{indent}<controlActProcess classCode=\"CACT\" moodCode=\"EVN\">{newline}",
                "{indent}{indent}<subject><registrationEvent classCode=\"REG\" moodCode=\"EVN\">{newline}",
                "{indent}{indent}{indent}<subject1><patient classCode=\"PAT\"><id extension=\"{patient_id}\" root=\"2.16.156.10011.1.12\"/></patient></subject1>{newline}",
                "{indent}{indent}</registrationEvent></subject>{newline}",
                "{indent}</controlActProcess>{newline}",
                "</{root}>"
            ),
            declaration = declaration,
            newline = newline,
            indent = indent,
            root = root,
            message_id = msg.id,
            patient_id = patient_id,
        )))
    }

    async fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        let content = match std::str::from_utf8(raw) {
            Ok(content) => content,
            Err(error) => {
                return Ok(ValidationResult::invalid(vec![ValidationError {
                    code: "HL7V3-001".to_string(),
                    message: format!("Invalid UTF-8: {}", error),
                    location: None,
                    severity: ErrorSeverity::Fatal,
                }]));
            }
        };

        match parse_hl7v3_xml(content) {
            Ok(_) => Ok(ValidationResult::valid()),
            Err(error) => Ok(ValidationResult::invalid(vec![ValidationError {
                code: "HL7V3-002".to_string(),
                message: error.to_string(),
                location: None,
                severity: ErrorSeverity::Error,
            }])),
        }
    }

    fn extract_message_type(&self, raw: &Bytes) -> Option<String> {
        let content = std::str::from_utf8(raw).ok()?;
        parse_hl7v3_xml(content)
            .ok()
            .and_then(|parsed| parsed.interaction_id.or(Some(parsed.root_element)))
    }
}

#[derive(Debug)]
struct ParsedHl7V3 {
    root_element: String,
    namespace: String,
    interaction_id: Option<String>,
    message_id: Option<String>,
    patient_id: Option<String>,
}

fn parse_hl7v3_xml(content: &str) -> HsbResult<ParsedHl7V3> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut root_element: Option<String> = None;
    let mut namespace: Option<String> = None;
    let mut interaction_id = None;
    let mut message_id = None;
    let mut patient_id = None;
    let mut path = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let name = local_name(event.name().as_ref());
                if root_element.is_none() {
                    root_element = Some(name.clone());
                    namespace = extract_namespace(&event, reader.decoder());
                }

                path.push(name.clone());
                if name == "interactionId" {
                    for attribute in event.attributes().with_checks(false).flatten() {
                        if attribute.key.as_ref() == b"extension" {
                            interaction_id = attribute
                                .decode_and_unescape_value(reader.decoder())
                                .ok()
                                .map(|value| value.to_string());
                        }
                    }
                }
                if name == "id" {
                    let extension = event
                        .attributes()
                        .with_checks(false)
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == b"extension")
                        .and_then(|attribute| {
                            attribute.decode_and_unescape_value(reader.decoder()).ok()
                        })
                        .map(|value| value.to_string());

                    if path.len() == 1 && message_id.is_none() {
                        message_id = extension;
                    } else if path.iter().any(|segment| segment == "patient")
                        && patient_id.is_none()
                    {
                        patient_id = extension;
                    }
                }
            }
            Ok(Event::Empty(event)) => {
                let name = local_name(event.name().as_ref());
                if root_element.is_none() {
                    root_element = Some(name.clone());
                    namespace = extract_namespace(&event, reader.decoder());
                }

                path.push(name.clone());
                if name == "interactionId" {
                    for attribute in event.attributes().with_checks(false).flatten() {
                        if attribute.key.as_ref() == b"extension" {
                            interaction_id = attribute
                                .decode_and_unescape_value(reader.decoder())
                                .ok()
                                .map(|value| value.to_string());
                        }
                    }
                }
                if name == "id" {
                    let extension = event
                        .attributes()
                        .with_checks(false)
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == b"extension")
                        .and_then(|attribute| {
                            attribute.decode_and_unescape_value(reader.decoder()).ok()
                        })
                        .map(|value| value.to_string());

                    if path.len() == 1 && message_id.is_none() {
                        message_id = extension;
                    } else if path.iter().any(|segment| segment == "patient")
                        && patient_id.is_none()
                    {
                        patient_id = extension;
                    }
                }
                path.pop();
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(HsbError::ParseError {
                    message: format!("HL7 v3 XML parse error: {}", error),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    let root_element = root_element.ok_or_else(|| HsbError::ParseError {
        message: "HL7 v3 XML missing root element".to_string(),
    })?;
    let namespace = namespace.unwrap_or_default();
    if namespace != "urn:hl7-org:v3" {
        return Err(HsbError::ParseError {
            message: format!("HL7 v3 namespace mismatch: '{}'", namespace),
        });
    }

    Ok(ParsedHl7V3 {
        root_element,
        namespace,
        interaction_id,
        message_id,
        patient_id,
    })
}

fn local_name(name: &[u8]) -> String {
    let decoded = String::from_utf8_lossy(name);
    decoded
        .rsplit(':')
        .next()
        .unwrap_or(decoded.as_ref())
        .to_string()
}

fn extract_namespace(
    event: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::encoding::Decoder,
) -> Option<String> {
    event
        .attributes()
        .with_checks(false)
        .flatten()
        .find_map(|attribute| {
            let key = String::from_utf8_lossy(attribute.key.as_ref()).to_string();
            if key == "xmlns" || key.starts_with("xmlns:") {
                attribute
                    .decode_and_unescape_value(decoder)
                    .ok()
                    .map(|value| value.to_string())
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        r#"<PRPA_IN201301UV02 xmlns="urn:hl7-org:v3">"#,
        r#"<id root="1.2.3" extension="MSG-1"/>"#,
        r#"<interactionId root="2.16.840.1.113883.1.6" extension="PRPA_IN201301UV02"/>"#,
        r#"<controlActProcess><subject><registrationEvent><subject1><patient><id root="9.9.9" extension="PAT-1"/></patient></subject1></registrationEvent></subject></controlActProcess>"#,
        r#"</PRPA_IN201301UV02>"#,
    );

    #[tokio::test]
    async fn adapter_parses_hl7v3_payload() {
        let adapter = Hl7V3Adapter::new();
        let message = adapter
            .parse(
                Bytes::from_static(SAMPLE.as_bytes()),
                &ParseOptions::default(),
            )
            .await
            .expect("parse should succeed");

        assert_eq!(message.protocol, ProtocolType::Hl7V3);
        assert_eq!(message.message_type.as_deref(), Some("PRPA_IN201301UV02"));
        assert_eq!(message.metadata.patient_id.as_deref(), Some("PAT-1"));
        assert_eq!(
            message
                .payload
                .as_ref()
                .and_then(|payload| payload.get("namespace"))
                .and_then(Value::as_str),
            Some("urn:hl7-org:v3")
        );
    }

    #[tokio::test]
    async fn adapter_rejects_non_hl7v3_namespace() {
        let adapter = Hl7V3Adapter::new();
        let result = adapter
            .validate(&Bytes::from_static(br#"<root xmlns="urn:not-hl7"/>"#))
            .await
            .expect("validate should succeed");

        assert!(!result.valid);
    }
}
