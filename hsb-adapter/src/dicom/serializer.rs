//! DICOM 消息序列化器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::Message;
use hsb_core::SerializeOptions;

use super::DicomConfig;

/// DICOM 序列化器
#[allow(dead_code)]
pub struct DicomSerializer {
    config: DicomConfig,
}

impl DicomSerializer {
    pub fn new() -> Self {
        Self {
            config: DicomConfig::default(),
        }
    }

    pub fn with_config(config: DicomConfig) -> Self {
        Self { config }
    }

    /// 将 Message 序列化为 DICOM 格式
    pub fn serialize(&self, msg: &Message, _options: &SerializeOptions) -> HsbResult<Bytes> {
        // 如果有原始报文，直接返回
        if !msg.raw_payload.is_empty() {
            return Ok(Bytes::from(msg.raw_payload.clone()));
        }

        // TODO: 从 JSON payload 构建 DICOM 对象
        Err(HsbError::SerializationError {
            message: "DICOM serialization from JSON not yet implemented".to_string(),
        })
    }
}

impl Default for DicomSerializer {
    fn default() -> Self {
        Self::new()
    }
}
