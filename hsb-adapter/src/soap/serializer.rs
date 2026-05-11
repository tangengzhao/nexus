//! SOAP 消息序列化器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::Message;
use hsb_core::SerializeOptions;

use super::{SoapConfig, SoapVersion};

/// SOAP 序列化器
pub struct SoapSerializer {
    config: SoapConfig,
}

impl SoapSerializer {
    pub fn new() -> Self {
        Self {
            config: SoapConfig::default(),
        }
    }

    pub fn with_config(config: SoapConfig) -> Self {
        Self { config }
    }

    /// 将 Message 序列化为 SOAP 格式
    pub fn serialize(&self, msg: &Message, options: &SerializeOptions) -> HsbResult<Bytes> {
        // 如果有原始报文，直接返回
        if !msg.raw_payload.is_empty() {
            return Ok(Bytes::from(msg.raw_payload.clone()));
        }

        let payload = msg
            .payload
            .as_ref()
            .ok_or_else(|| HsbError::SerializationError {
                message: "No payload to serialize".to_string(),
            })?;

        let body_content = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let envelope = self.build_envelope(body_content, options);
        Ok(Bytes::from(envelope))
    }

    /// 构建 SOAP Envelope
    fn build_envelope(&self, body_content: &str, options: &SerializeOptions) -> String {
        let ns = self.config.version.namespace();
        let indent = if options.pretty_print { "  " } else { "" };
        let newline = if options.pretty_print { "\n" } else { "" };

        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>{newline}<soap:Envelope xmlns:soap="{ns}">{newline}{indent}<soap:Header/>{newline}{indent}<soap:Body>{newline}{indent}{indent}{body}{newline}{indent}</soap:Body>{newline}</soap:Envelope>"#,
            ns = ns,
            body = body_content,
            indent = indent,
            newline = newline
        )
    }

    /// 构建 SOAP Fault
    pub fn build_fault(&self, fault_code: &str, fault_string: &str) -> String {
        match self.config.version {
            SoapVersion::Soap11 => format!(
                r#"<soap:Fault>
  <faultcode>{}</faultcode>
  <faultstring>{}</faultstring>
</soap:Fault>"#,
                fault_code, fault_string
            ),
            SoapVersion::Soap12 => format!(
                r#"<soap:Fault>
  <soap:Code>
    <soap:Value>{}</soap:Value>
  </soap:Code>
  <soap:Reason>
    <soap:Text xml:lang="en">{}</soap:Text>
  </soap:Reason>
</soap:Fault>"#,
                fault_code, fault_string
            ),
        }
    }
}

impl Default for SoapSerializer {
    fn default() -> Self {
        Self::new()
    }
}
