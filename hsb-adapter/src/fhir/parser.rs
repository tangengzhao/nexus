//! FHIR 消息解析器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use hsb_core::{ErrorSeverity, ParseOptions, ValidationError, ValidationResult};
use hsb_core::{Message, MessageBuilder};
use serde_json::Value;

use super::FhirConfig;

/// FHIR 解析器
#[allow(dead_code)]
pub struct FhirParser {
    config: FhirConfig,
}

impl FhirParser {
    pub fn new() -> Self {
        Self {
            config: FhirConfig::default(),
        }
    }

    pub fn with_config(config: FhirConfig) -> Self {
        Self { config }
    }

    /// 解析 FHIR 资源
    pub fn parse(&self, raw: Bytes, _options: &ParseOptions) -> HsbResult<Message> {
        let content = String::from_utf8(raw.to_vec()).map_err(|e| HsbError::ParseError {
            message: format!("Invalid UTF-8: {}", e),
        })?;

        // 解析 JSON
        let json: Value = serde_json::from_str(&content).map_err(|e| HsbError::ParseError {
            message: format!("Invalid JSON: {}", e),
        })?;

        // 获取资源类型
        let resource_type = json
            .get("resourceType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HsbError::ParseError {
                message: "Missing resourceType field".to_string(),
            })?;

        // 获取资源 ID
        let _resource_id = json.get("id").and_then(|v| v.as_str());

        // 提取患者引用
        let patient_id = self.extract_patient_reference(&json);

        // 提取 Encounter 引用
        let encounter_id = self.extract_encounter_reference(&json);

        // 构建消息
        let mut builder = MessageBuilder::new()
            .source_system("FHIR")
            .protocol(ProtocolType::FhirR5)
            .message_type(resource_type)
            .payload(json)
            .raw_payload(raw);

        if let Some(pid) = patient_id {
            builder = builder.patient_id(pid);
        }

        if let Some(eid) = encounter_id {
            builder = builder.visit_id(eid);
        }

        builder.build()
    }

    /// 验证 FHIR 资源
    pub fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        let content = match String::from_utf8(raw.to_vec()) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ValidationResult::invalid(vec![ValidationError {
                    code: "FHIR-001".to_string(),
                    message: format!("Invalid UTF-8: {}", e),
                    location: None,
                    severity: ErrorSeverity::Fatal,
                }]));
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ValidationResult::invalid(vec![ValidationError {
                    code: "FHIR-002".to_string(),
                    message: format!("Invalid JSON: {}", e),
                    location: None,
                    severity: ErrorSeverity::Fatal,
                }]));
            }
        };

        let mut errors = Vec::new();

        // 检查 resourceType
        if json.get("resourceType").is_none() {
            errors.push(ValidationError {
                code: "FHIR-003".to_string(),
                message: "Missing required field: resourceType".to_string(),
                location: Some("resourceType".to_string()),
                severity: ErrorSeverity::Error,
            });
        }

        if errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(errors))
        }
    }

    /// 提取资源类型
    pub fn extract_resource_type(&self, raw: &Bytes) -> Option<String> {
        let content = String::from_utf8(raw.to_vec()).ok()?;
        let json: Value = serde_json::from_str(&content).ok()?;
        json.get("resourceType")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// 提取患者引用
    fn extract_patient_reference(&self, json: &Value) -> Option<String> {
        // 尝试多种路径
        let paths = [vec!["subject", "reference"], vec!["patient", "reference"]];

        for path in paths {
            if let Some(reference) = self.get_nested_str(json, &path) {
                // 解析引用格式：Patient/123
                if let Some(id) = reference.strip_prefix("Patient/") {
                    return Some(id.to_string());
                }
                return Some(reference.to_string());
            }
        }

        None
    }

    /// 提取 Encounter 引用
    fn extract_encounter_reference(&self, json: &Value) -> Option<String> {
        let reference = self.get_nested_str(json, &["encounter", "reference"])?;
        if let Some(id) = reference.strip_prefix("Encounter/") {
            return Some(id.to_string());
        }
        Some(reference.to_string())
    }

    /// 获取嵌套字符串值
    fn get_nested_str<'a>(&self, json: &'a Value, path: &[&str]) -> Option<&'a str> {
        let mut current = json;
        for key in path {
            current = current.get(*key)?;
        }
        current.as_str()
    }
}

impl Default for FhirParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patient() {
        let patient_json = r#"{
            "resourceType": "Patient",
            "id": "12345",
            "name": [{"family": "Doe", "given": ["John"]}]
        }"#;

        let parser = FhirParser::new();
        let result = parser.parse(Bytes::from(patient_json), &ParseOptions::default());

        assert!(result.is_ok());
        let msg = result.expect("Should parse");
        assert_eq!(msg.message_type, Some("Patient".to_string()));
    }

    #[test]
    fn test_extract_resource_type() {
        let json = r#"{"resourceType": "Observation", "id": "123"}"#;
        let parser = FhirParser::new();

        let rt = parser.extract_resource_type(&Bytes::from(json));
        assert_eq!(rt, Some("Observation".to_string()));
    }
}
