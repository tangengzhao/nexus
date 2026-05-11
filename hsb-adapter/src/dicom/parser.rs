//! DICOM 消息解析器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use hsb_core::{ErrorSeverity, ParseOptions, ValidationError, ValidationResult};
use hsb_core::{Message, MessageBuilder};
use serde_json::{Value, json};

use super::DicomConfig;

/// DICOM 解析器
#[allow(dead_code)]
pub struct DicomParser {
    config: DicomConfig,
}

impl DicomParser {
    pub fn new() -> Self {
        Self {
            config: DicomConfig::default(),
        }
    }

    pub fn with_config(config: DicomConfig) -> Self {
        Self { config }
    }

    /// 解析 DICOM 数据
    pub fn parse(&self, raw: Bytes, _options: &ParseOptions) -> HsbResult<Message> {
        // 使用 dicom-rs 解析
        let obj = dicom_object::from_reader(&raw[..]).map_err(|e| HsbError::ParseError {
            message: format!("DICOM parse error: {}", e),
        })?;

        // 提取关键元素
        let mut payload = serde_json::Map::new();

        // Patient ID
        if let Ok(element) = obj.element_by_name("PatientID") {
            if let Ok(value) = element.to_str() {
                payload.insert("PatientID".to_string(), json!(value.to_string()));
            }
        }

        // Patient Name
        if let Ok(element) = obj.element_by_name("PatientName") {
            if let Ok(value) = element.to_str() {
                payload.insert("PatientName".to_string(), json!(value.to_string()));
            }
        }

        // Study Instance UID
        if let Ok(element) = obj.element_by_name("StudyInstanceUID") {
            if let Ok(value) = element.to_str() {
                payload.insert("StudyInstanceUID".to_string(), json!(value.to_string()));
            }
        }

        // Series Instance UID
        if let Ok(element) = obj.element_by_name("SeriesInstanceUID") {
            if let Ok(value) = element.to_str() {
                payload.insert("SeriesInstanceUID".to_string(), json!(value.to_string()));
            }
        }

        // SOP Instance UID
        if let Ok(element) = obj.element_by_name("SOPInstanceUID") {
            if let Ok(value) = element.to_str() {
                payload.insert("SOPInstanceUID".to_string(), json!(value.to_string()));
            }
        }

        // SOP Class UID
        let sop_class = if let Ok(element) = obj.element_by_name("SOPClassUID") {
            element.to_str().ok().map(|s| s.to_string())
        } else {
            None
        };

        // Modality
        if let Ok(element) = obj.element_by_name("Modality") {
            if let Ok(value) = element.to_str() {
                payload.insert("Modality".to_string(), json!(value.to_string()));
            }
        }

        // Study Date
        if let Ok(element) = obj.element_by_name("StudyDate") {
            if let Ok(value) = element.to_str() {
                payload.insert("StudyDate".to_string(), json!(value.to_string()));
            }
        }

        // Accession Number
        if let Ok(element) = obj.element_by_name("AccessionNumber") {
            if let Ok(value) = element.to_str() {
                payload.insert("AccessionNumber".to_string(), json!(value.to_string()));
            }
        }

        // 获取患者 ID
        let patient_id = payload
            .get("PatientID")
            .and_then(|v| v.as_str())
            .map(String::from);

        // 构建消息
        let mut builder = MessageBuilder::new()
            .source_system("DICOM")
            .protocol(ProtocolType::Dicom)
            .payload(Value::Object(payload))
            .raw_payload(raw);

        if let Some(sop) = sop_class {
            builder = builder.message_type(sop);
        }

        if let Some(pid) = patient_id {
            builder = builder.patient_id(pid);
        }

        builder.build()
    }

    /// 验证 DICOM 数据
    pub fn validate(&self, raw: &Bytes) -> HsbResult<ValidationResult> {
        match dicom_object::from_reader(&raw[..]) {
            Ok(_) => Ok(ValidationResult::valid()),
            Err(e) => Ok(ValidationResult::invalid(vec![ValidationError {
                code: "DICOM-001".to_string(),
                message: format!("Invalid DICOM format: {}", e),
                location: None,
                severity: ErrorSeverity::Fatal,
            }])),
        }
    }

    /// 提取 SOP Class UID
    pub fn extract_sop_class(&self, raw: &Bytes) -> Option<String> {
        let obj = dicom_object::from_reader(&raw[..]).ok()?;
        obj.element_by_name("SOPClassUID")
            .ok()
            .and_then(|e| e.to_str().ok())
            .map(|s| s.to_string())
    }
}

impl Default for DicomParser {
    fn default() -> Self {
        Self::new()
    }
}
