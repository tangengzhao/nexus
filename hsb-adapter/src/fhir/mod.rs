//! HSB FHIR R5 协议适配器
//!
//! 支持 HL7 FHIR R5 标准的解析和生成。

mod bundle;
mod parser;
mod resources;
mod serializer;

pub use bundle::*;
pub use parser::FhirParser;
pub use resources::*;
pub use serializer::FhirSerializer;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, ProtocolType};
use hsb_core::Message;
use hsb_core::{ParseOptions, ProtocolAdapter, SerializeOptions, ValidationResult};

/// FHIR R5 适配器
#[allow(dead_code)]
pub struct FhirR5Adapter {
    parser: FhirParser,
    serializer: FhirSerializer,
    config: FhirConfig,
}

impl FhirR5Adapter {
    pub fn new() -> Self {
        Self {
            parser: FhirParser::new(),
            serializer: FhirSerializer::new(),
            config: FhirConfig::default(),
        }
    }

    pub fn with_config(config: FhirConfig) -> Self {
        Self {
            parser: FhirParser::with_config(config.clone()),
            serializer: FhirSerializer::with_config(config.clone()),
            config,
        }
    }
}

impl Default for FhirR5Adapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAdapter for FhirR5Adapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::FhirR5
    }

    fn name(&self) -> &str {
        "FHIR R5 Adapter"
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
        self.parser.extract_resource_type(raw)
    }
}

/// FHIR 配置
#[derive(Debug, Clone)]
pub struct FhirConfig {
    /// FHIR 版本
    pub version: FhirVersion,
    /// 是否严格验证
    pub strict_validation: bool,
    /// 服务器 URL
    pub server_url: Option<String>,
    /// 默认格式
    pub default_format: FhirFormat,
    /// 是否启用 Profile 验证
    pub validate_profiles: bool,
}

impl Default for FhirConfig {
    fn default() -> Self {
        Self {
            version: FhirVersion::R5,
            strict_validation: false,
            server_url: None,
            default_format: FhirFormat::Json,
            validate_profiles: false,
        }
    }
}

/// FHIR 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FhirVersion {
    Dstu2,
    Stu3,
    R4,
    R4B,
    R5,
}

impl FhirVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dstu2 => "1.0.2",
            Self::Stu3 => "3.0.2",
            Self::R4 => "4.0.1",
            Self::R4B => "4.3.0",
            Self::R5 => "5.0.0",
        }
    }
}

/// FHIR 格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FhirFormat {
    Json,
    Xml,
}

/// FHIR 资源类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceType {
    Patient,
    Encounter,
    Observation,
    DiagnosticReport,
    Medication,
    MedicationRequest,
    Procedure,
    Condition,
    AllergyIntolerance,
    Immunization,
    ServiceRequest,
    Task,
    Bundle,
    OperationOutcome,
    Other(String),
}

impl ResourceType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Patient" => Self::Patient,
            "Encounter" => Self::Encounter,
            "Observation" => Self::Observation,
            "DiagnosticReport" => Self::DiagnosticReport,
            "Medication" => Self::Medication,
            "MedicationRequest" => Self::MedicationRequest,
            "Procedure" => Self::Procedure,
            "Condition" => Self::Condition,
            "AllergyIntolerance" => Self::AllergyIntolerance,
            "Immunization" => Self::Immunization,
            "ServiceRequest" => Self::ServiceRequest,
            "Task" => Self::Task,
            "Bundle" => Self::Bundle,
            "OperationOutcome" => Self::OperationOutcome,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Patient => "Patient",
            Self::Encounter => "Encounter",
            Self::Observation => "Observation",
            Self::DiagnosticReport => "DiagnosticReport",
            Self::Medication => "Medication",
            Self::MedicationRequest => "MedicationRequest",
            Self::Procedure => "Procedure",
            Self::Condition => "Condition",
            Self::AllergyIntolerance => "AllergyIntolerance",
            Self::Immunization => "Immunization",
            Self::ServiceRequest => "ServiceRequest",
            Self::Task => "Task",
            Self::Bundle => "Bundle",
            Self::OperationOutcome => "OperationOutcome",
            Self::Other(s) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = FhirR5Adapter::new();
        assert_eq!(adapter.protocol(), ProtocolType::FhirR5);
        assert_eq!(adapter.name(), "FHIR R5 Adapter");
    }
}
