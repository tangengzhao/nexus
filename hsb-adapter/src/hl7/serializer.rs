//! HL7 消息序列化器

use bytes::Bytes;
use chrono::Utc;
use hsb_common::{HsbError, HsbResult, constants::hl7::*};
use hsb_core::Message;
use hsb_core::SerializeOptions;

use super::Hl7Config;

/// HL7 序列化器
pub struct Hl7Serializer {
    config: Hl7Config,
}

impl Hl7Serializer {
    pub fn new() -> Self {
        Self {
            config: Hl7Config::default(),
        }
    }

    pub fn with_config(config: Hl7Config) -> Self {
        Self { config }
    }

    /// 将 Message 序列化为 HL7 格式
    pub fn serialize(&self, msg: &Message, _options: &SerializeOptions) -> HsbResult<Bytes> {
        let payload = msg
            .payload
            .as_ref()
            .ok_or_else(|| HsbError::SerializationError {
                message: "No payload to serialize".to_string(),
            })?;

        // 如果原始报文存在且未修改，直接返回
        if !msg.raw_payload.is_empty() {
            // TODO: 检测是否有修改
            return Ok(Bytes::from(msg.raw_payload.clone()));
        }

        // 从 JSON 构建 HL7 消息
        let mut segments = Vec::new();

        // 构建 MSH 段
        let msh = self.build_msh(msg)?;
        segments.push(msh);

        // 构建其他段（根据 payload）
        if let Some(obj) = payload.as_object() {
            for (key, value) in obj {
                if key == "MSH" {
                    continue; // 已处理
                }
                if let Some(seg_str) = self.json_to_segment(key, value) {
                    segments.push(seg_str);
                }
            }
        }

        let result = segments.join(&SEGMENT_SEPARATOR.to_string());
        Ok(Bytes::from(result))
    }

    /// 构建 MSH 段
    fn build_msh(&self, msg: &Message) -> HsbResult<String> {
        let now = Utc::now();
        let timestamp = now.format(&self.config.timestamp_format).to_string();

        let sending_app = msg.metadata.sending_application.as_deref().unwrap_or("HSB");
        let sending_fac = msg.metadata.sending_facility.as_deref().unwrap_or("");
        let receiving_app = msg.metadata.receiving_application.as_deref().unwrap_or("");
        let receiving_fac = msg.metadata.receiving_facility.as_deref().unwrap_or("");
        let message_type = msg.message_type.as_deref().unwrap_or("ADT^A01");
        let default_control_id = msg.id.to_string();
        let control_id = msg
            .metadata
            .message_control_id
            .as_deref()
            .unwrap_or(&default_control_id);
        let processing_id = msg.metadata.processing_id.as_deref().unwrap_or("P");
        let version = self.config.version.as_str();

        Ok(format!(
            "MSH|^~\\&|{}|{}|{}|{}|{}||{}|{}|{}|{}",
            sending_app,
            sending_fac,
            receiving_app,
            receiving_fac,
            timestamp,
            message_type,
            control_id,
            processing_id,
            version
        ))
    }

    /// 将 JSON 转换为段字符串
    fn json_to_segment(&self, name: &str, value: &serde_json::Value) -> Option<String> {
        let obj = value.as_object()?;
        let mut fields = vec![String::new(); 50]; // 预分配足够的字段

        for (key, val) in obj {
            // 解析字段名（如 PID_3）
            let parts: Vec<&str> = key.split('_').collect();
            if parts.len() != 2 {
                continue;
            }

            let field_index: usize = parts[1].parse().ok()?;
            if field_index == 0 || field_index > 49 {
                continue;
            }

            let field_value = match val {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(&COMPONENT_SEPARATOR.to_string()),
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };

            fields[field_index - 1] = field_value;
        }

        // 找到最后一个非空字段
        let last_non_empty = fields.iter().rposition(|s| !s.is_empty()).unwrap_or(0);
        let fields = &fields[..=last_non_empty];

        if fields.is_empty() {
            return None;
        }

        Some(format!(
            "{}|{}",
            name,
            fields.join(&FIELD_SEPARATOR.to_string())
        ))
    }

    /// 添加 MLLP 包装
    pub fn wrap_mllp(&self, data: Bytes) -> Bytes {
        let mut result = Vec::with_capacity(data.len() + 3);
        result.push(MLLP_START_BLOCK);
        result.extend_from_slice(&data);
        result.push(MLLP_END_BLOCK);
        result.push(MLLP_CARRIAGE_RETURN);
        Bytes::from(result)
    }
}

impl Default for Hl7Serializer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hsb_common::ProtocolType;
    use hsb_core::MessageBuilder;

    #[test]
    fn test_mllp_wrap() {
        let serializer = Hl7Serializer::new();
        let data = Bytes::from("MSH|^~\\&|...");
        let wrapped = serializer.wrap_mllp(data);

        assert_eq!(wrapped[0], MLLP_START_BLOCK);
        assert_eq!(wrapped[wrapped.len() - 2], MLLP_END_BLOCK);
        assert_eq!(wrapped[wrapped.len() - 1], MLLP_CARRIAGE_RETURN);
    }
}
