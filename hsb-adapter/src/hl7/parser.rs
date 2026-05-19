//! HL7 消息解析器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType, constants::hl7::*};
use hsb_core::{ErrorSeverity, ParseOptions, ValidationError, ValidationResult};
use hsb_core::{Message, MessageBuilder};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

use super::Hl7Config;

/// HL7 解析器
#[allow(dead_code)]
pub struct Hl7Parser {
    config: Hl7Config,
}

impl Hl7Parser {
    pub fn new() -> Self {
        Self {
            config: Hl7Config::default(),
        }
    }

    pub fn with_config(config: Hl7Config) -> Self {
        Self { config }
    }

    /// 解析 HL7 消息
    pub fn parse(&self, raw: Bytes, _options: &ParseOptions) -> HsbResult<Message> {
        let content = String::from_utf8(raw.to_vec()).map_err(|e| HsbError::ParseError {
            message: format!("Invalid UTF-8: {}", e),
        })?;

        // 移除 MLLP 包装（如果存在）
        let content = self.strip_mllp(&content);

        // 解析段
        let segments = self.parse_segments(&content)?;

        // 获取 MSH 段
        let msh = segments.first().ok_or_else(|| HsbError::ParseError {
            message: "Missing MSH segment".to_string(),
        })?;

        if msh.name != "MSH" {
            return Err(HsbError::ParseError {
                message: format!("First segment must be MSH, got: {}", msh.name),
            });
        }

        // 提取 MSH 字段
        let sending_app = msh.get_field(3).unwrap_or_default();
        let sending_facility = msh.get_field(4).unwrap_or_default();
        let receiving_app = msh.get_field(5).unwrap_or_default();
        let receiving_facility = msh.get_field(6).unwrap_or_default();
        let message_type = msh.get_field(9).unwrap_or_default();
        let message_control_id = msh.get_field(10).unwrap_or_default();
        let processing_id = msh.get_field(11).unwrap_or_default();
        let _version_id = msh.get_field(12).unwrap_or_default();

        // 构建 JSON payload
        let payload = self.segments_to_json(&segments);

        // 构建源系统 ID
        let source_system = if sending_facility.is_empty() {
            sending_app.to_string()
        } else {
            format!("{}_{}", sending_facility, sending_app)
        };

        // 构建目标系统 ID
        let target_system = if receiving_facility.is_empty() {
            if receiving_app.is_empty() {
                None
            } else {
                Some(receiving_app.to_string())
            }
        } else {
            Some(format!("{}_{}", receiving_facility, receiving_app))
        };

        // 提取患者信息（如果有 PID 段）
        let patient_id = segments
            .iter()
            .find(|s| s.name == "PID")
            .and_then(|pid| pid.get_field(3))
            .map(|s| s.to_string());

        let visit_id = segments
            .iter()
            .find(|s| s.name == "PV1")
            .and_then(|pv1| pv1.get_field(19))
            .map(|s| s.to_string());

        // 构建消息
        let mut builder = MessageBuilder::new()
            .source_system(source_system)
            .protocol(ProtocolType::Hl7V2)
            .message_type(message_type)
            .payload(payload)
            .raw_payload(raw);

        if let Some(target) = target_system {
            builder = builder.target_system(target);
        }

        if let Some(pid) = patient_id {
            builder = builder.patient_id(pid);
        }

        if let Some(vid) = visit_id {
            builder = builder.visit_id(vid);
        }

        let mut msg = builder.build()?;

        // 设置元数据
        msg.metadata.sending_application = Some(sending_app.to_string());
        msg.metadata.sending_facility = Some(sending_facility.to_string());
        msg.metadata.receiving_application = Some(receiving_app.to_string());
        msg.metadata.receiving_facility = Some(receiving_facility.to_string());
        msg.metadata.message_control_id = Some(message_control_id.to_string());
        msg.metadata.processing_id = Some(processing_id.to_string());

        Ok(msg)
    }

    /// 验证 HL7 消息
    pub fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        let content = match String::from_utf8(raw.to_vec()) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ValidationResult::invalid(vec![ValidationError {
                    code: "HL7-001".to_string(),
                    message: format!("Invalid UTF-8 encoding: {}", e),
                    location: None,
                    severity: ErrorSeverity::Fatal,
                }]));
            }
        };

        let content = self.strip_mllp(&content);
        let mut errors = Vec::new();

        // 检查是否以 MSH 开头
        if !content.starts_with("MSH") {
            errors.push(ValidationError {
                code: "HL7-002".to_string(),
                message: "Message must start with MSH segment".to_string(),
                location: Some("Line 1".to_string()),
                severity: ErrorSeverity::Fatal,
            });
        }

        // 检查段分隔符
        let segments: Vec<&str> = content.split(SEGMENT_SEPARATOR).collect();
        if segments.is_empty() {
            errors.push(ValidationError {
                code: "HL7-003".to_string(),
                message: "No segments found".to_string(),
                location: None,
                severity: ErrorSeverity::Fatal,
            });
        }

        // 检查 MSH-9（消息类型）
        if let Some(msh) = segments.first() {
            let fields: Vec<&str> = msh.split(FIELD_SEPARATOR).collect();
            if fields.len() < 10 {
                errors.push(ValidationError {
                    code: "HL7-004".to_string(),
                    message: "MSH segment incomplete".to_string(),
                    location: Some("MSH".to_string()),
                    severity: ErrorSeverity::Error,
                });
            }
        }

        if errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(errors))
        }
    }

    /// 提取消息类型
    pub fn extract_message_type(&self, raw: &Bytes) -> Option<String> {
        let content = String::from_utf8(raw.to_vec()).ok()?;
        let content = self.strip_mllp(&content);

        // 找到 MSH-9
        let lines: Vec<&str> = content.split(SEGMENT_SEPARATOR).collect();
        let msh = lines.first()?;
        let fields: Vec<&str> = msh.split(FIELD_SEPARATOR).collect();

        // MSH-9 是第 9 个字段（索引 8，因为分隔符本身占位）
        fields.get(8).map(|s| s.to_string())
    }

    /// 移除 MLLP 包装
    fn strip_mllp(&self, content: &str) -> String {
        let mut result = content.to_string();

        // 移除开始字符
        if result.starts_with(char::from(MLLP_START_BLOCK)) {
            result = result[1..].to_string();
        }

        // 移除结束字符
        if result.ends_with(char::from(MLLP_CARRIAGE_RETURN)) {
            result.pop();
        }
        if result.ends_with(char::from(MLLP_END_BLOCK)) {
            result.pop();
        }

        result.trim().to_string()
    }

    /// 解析段
    fn parse_segments(&self, content: &str) -> HsbResult<Vec<Segment>> {
        let mut segments = Vec::new();

        for line in content.split(SEGMENT_SEPARATOR) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let segment = self.parse_segment(line)?;
            segments.push(segment);
        }

        Ok(segments)
    }

    /// 解析单个段
    fn parse_segment(&self, line: &str) -> HsbResult<Segment> {
        if line.len() < 3 {
            return Err(HsbError::ParseError {
                message: format!("Segment too short: {}", line),
            });
        }

        let name = &line[0..3];
        let fields: Vec<String> = if name == "MSH" {
            // MSH 特殊处理：第一个字段是分隔符
            let mut result = vec!["|".to_string()];
            result.extend(line[4..].split(FIELD_SEPARATOR).map(String::from));
            result
        } else {
            line[4..].split(FIELD_SEPARATOR).map(String::from).collect()
        };

        Ok(Segment {
            name: name.to_string(),
            fields,
        })
    }

    /// 将段列表转换为 JSON
    fn segments_to_json(&self, segments: &[Segment]) -> Value {
        let mut root = Map::new();
        let mut segment_arrays: HashMap<String, Vec<Value>> = HashMap::new();

        for segment in segments {
            let seg_json = segment.to_json();
            segment_arrays
                .entry(segment.name.clone())
                .or_default()
                .push(seg_json);
        }

        for (name, values) in segment_arrays {
            if values.len() == 1 {
                root.insert(name, values.into_iter().next().unwrap_or(Value::Null));
            } else {
                root.insert(name, Value::Array(values));
            }
        }

        Value::Object(root)
    }
}

impl Default for Hl7Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// HL7 段
#[derive(Debug, Clone)]
pub struct Segment {
    pub name: String,
    pub fields: Vec<String>,
}

impl Segment {
    /// 获取字段（1-based 索引）
    pub fn get_field(&self, index: usize) -> Option<&str> {
        if index == 0 {
            return None;
        }
        self.fields.get(index - 1).map(|s| s.as_str())
    }

    /// 转换为 JSON
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();

        for (i, field) in self.fields.iter().enumerate() {
            if !field.is_empty() {
                let key = format!("{}_{}", self.name, i + 1);

                // 处理组件分隔符
                if field.contains(COMPONENT_SEPARATOR) {
                    let components: Vec<&str> = field.split(COMPONENT_SEPARATOR).collect();
                    map.insert(key, json!(components));
                } else {
                    map.insert(key, json!(field));
                }
            }
        }

        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let msg = "MSH|^~\\&|SENDING_APP|SENDING_FAC|RECEIVING_APP|RECEIVING_FAC|20230101120000||ADT^A01|MSG00001|P|2.5.1\rPID|||12345||Doe^John||19800101|M";
        let parser = Hl7Parser::new();

        let result = parser.parse(Bytes::from(msg), &ParseOptions::default());
        assert!(result.is_ok());

        let message = result.expect("Should parse");
        assert_eq!(message.message_type, Some("ADT^A01".to_string()));
    }

    #[test]
    fn test_extract_message_type() {
        let msg = "MSH|^~\\&|APP|FAC|APP2|FAC2|20230101||ORM^O01|123|P|2.5";
        let parser = Hl7Parser::new();

        let msg_type = parser.extract_message_type(&Bytes::from(msg));
        assert_eq!(msg_type, Some("ORM^O01".to_string()));
    }
}
