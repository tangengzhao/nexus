//! FHIR 消息序列化器

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::Message;
use hsb_core::SerializeOptions;

use super::FhirConfig;

/// FHIR 序列化器
#[allow(dead_code)]
pub struct FhirSerializer {
    config: FhirConfig,
}

impl FhirSerializer {
    pub fn new() -> Self {
        Self {
            config: FhirConfig::default(),
        }
    }

    pub fn with_config(config: FhirConfig) -> Self {
        Self { config }
    }

    /// 将 Message 序列化为 FHIR JSON
    pub fn serialize(&self, msg: &Message, options: &SerializeOptions) -> HsbResult<Bytes> {
        let payload = msg
            .payload
            .as_ref()
            .ok_or_else(|| HsbError::SerializationError {
                message: "No payload to serialize".to_string(),
            })?;

        let json_str = if options.pretty_print {
            serde_json::to_string_pretty(payload)
        } else {
            serde_json::to_string(payload)
        }
        .map_err(|e| HsbError::SerializationError {
            message: format!("JSON serialization error: {}", e),
        })?;

        Ok(Bytes::from(json_str))
    }
}

impl Default for FhirSerializer {
    fn default() -> Self {
        Self::new()
    }
}
