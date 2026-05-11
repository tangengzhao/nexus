//! HSB DICOM 协议适配器
//!
//! 支持 DICOM 协议的解析和生成，包括 DIMSE 服务。

mod parser;
mod serializer;
mod services;
mod tags;

pub use parser::DicomParser;
pub use serializer::DicomSerializer;
pub use services::*;
pub use tags::*;

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, ProtocolType};
use hsb_core::Message;
use hsb_core::{ParseOptions, ProtocolAdapter, SerializeOptions, ValidationResult};

/// DICOM 适配器
#[allow(dead_code)]
pub struct DicomAdapter {
    parser: DicomParser,
    serializer: DicomSerializer,
    config: DicomConfig,
}

impl DicomAdapter {
    pub fn new() -> Self {
        Self {
            parser: DicomParser::new(),
            serializer: DicomSerializer::new(),
            config: DicomConfig::default(),
        }
    }

    pub fn with_config(config: DicomConfig) -> Self {
        Self {
            parser: DicomParser::with_config(config.clone()),
            serializer: DicomSerializer::with_config(config.clone()),
            config,
        }
    }
}

impl Default for DicomAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolAdapter for DicomAdapter {
    fn protocol(&self) -> ProtocolType {
        ProtocolType::Dicom
    }

    fn name(&self) -> &str {
        "DICOM Adapter"
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
        self.parser.extract_sop_class(raw)
    }
}

/// DICOM 配置
#[derive(Debug, Clone)]
pub struct DicomConfig {
    /// AE Title
    pub ae_title: String,
    /// 最大 PDU 长度
    pub max_pdu_length: u32,
    /// 超时（秒）
    pub timeout_secs: u64,
    /// 支持的传输语法
    pub transfer_syntaxes: Vec<String>,
    /// 支持的 SOP 类
    pub sop_classes: Vec<String>,
}

impl Default for DicomConfig {
    fn default() -> Self {
        Self {
            ae_title: "HSB_DICOM".to_string(),
            max_pdu_length: 16384,
            timeout_secs: 30,
            transfer_syntaxes: vec![
                "1.2.840.10008.1.2".to_string(),   // Implicit VR Little Endian
                "1.2.840.10008.1.2.1".to_string(), // Explicit VR Little Endian
                "1.2.840.10008.1.2.2".to_string(), // Explicit VR Big Endian
            ],
            sop_classes: vec![
                "1.2.840.10008.5.1.4.1.1.2".to_string(), // CT Image Storage
                "1.2.840.10008.5.1.4.1.1.4".to_string(), // MR Image Storage
                "1.2.840.10008.5.1.4.31".to_string(),    // Modality Worklist
                "1.2.840.10008.1.1".to_string(),         // Verification
            ],
        }
    }
}

/// DIMSE 服务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimseService {
    /// C-STORE
    CStore,
    /// C-FIND
    CFind,
    /// C-MOVE
    CMove,
    /// C-GET
    CGet,
    /// C-ECHO
    CEcho,
    /// N-EVENT-REPORT
    NEventReport,
    /// N-GET
    NGet,
    /// N-SET
    NSet,
    /// N-ACTION
    NAction,
    /// N-CREATE
    NCreate,
    /// N-DELETE
    NDelete,
}

impl DimseService {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CStore => "C-STORE",
            Self::CFind => "C-FIND",
            Self::CMove => "C-MOVE",
            Self::CGet => "C-GET",
            Self::CEcho => "C-ECHO",
            Self::NEventReport => "N-EVENT-REPORT",
            Self::NGet => "N-GET",
            Self::NSet => "N-SET",
            Self::NAction => "N-ACTION",
            Self::NCreate => "N-CREATE",
            Self::NDelete => "N-DELETE",
        }
    }
}

/// SOP 类 UID
pub mod sop_class {
    pub const VERIFICATION: &str = "1.2.840.10008.1.1";
    pub const CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
    pub const MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
    pub const US_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.6.1";
    pub const SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";
    pub const PATIENT_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.1.1";
    pub const PATIENT_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.1.2";
    pub const STUDY_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
    pub const STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
    pub const MODALITY_WORKLIST: &str = "1.2.840.10008.5.1.4.31";
    pub const MPPS: &str = "1.2.840.10008.3.1.2.3.3";
    pub const STORAGE_COMMITMENT: &str = "1.2.840.10008.1.20.1";
}

/// 传输语法 UID
pub mod transfer_syntax {
    pub const IMPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2";
    pub const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";
    pub const EXPLICIT_VR_BIG_ENDIAN: &str = "1.2.840.10008.1.2.2";
    pub const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
    pub const JPEG_LOSSLESS: &str = "1.2.840.10008.1.2.4.70";
    pub const JPEG_2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
    pub const JPEG_2000: &str = "1.2.840.10008.1.2.4.91";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = DicomAdapter::new();
        assert_eq!(adapter.protocol(), ProtocolType::Dicom);
        assert_eq!(adapter.name(), "DICOM Adapter");
    }
}
