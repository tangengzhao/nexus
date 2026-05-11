//! HL7 ACK（确认）消息处理

use super::segment::{MessageType, MshSegment};
use super::types::Timestamp;
use bytes::Bytes;
use hsb_common::constants::hl7::*;
use hsb_core::Message;
use serde::{Deserialize, Serialize};

/// ACK 确认码
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AckCode {
    /// 应用接受（Application Accept）
    AA,
    /// 应用错误（Application Error）
    AE,
    /// 应用拒绝（Application Reject）
    AR,
    /// 提交接受（Commit Accept）
    CA,
    /// 提交错误（Commit Error）
    CE,
    /// 提交拒绝（Commit Reject）
    CR,
}

impl AckCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AA => "AA",
            Self::AE => "AE",
            Self::AR => "AR",
            Self::CA => "CA",
            Self::CE => "CE",
            Self::CR => "CR",
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::AA | Self::CA)
    }
}

/// ACK 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckMessage {
    /// MSH 段
    pub msh: MshSegment,
    /// MSA 段
    pub msa: MsaSegment,
    /// ERR 段（可选，当有错误时）
    pub err: Option<ErrSegment>,
}

/// MSA 段（消息确认）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MsaSegment {
    /// MSA-1: 确认码
    pub acknowledgment_code: String,
    /// MSA-2: 消息控制 ID
    pub message_control_id: String,
    /// MSA-3: 文本消息
    pub text_message: Option<String>,
    /// MSA-4: 期望序列号
    pub expected_sequence_number: Option<String>,
    /// MSA-5: 延迟确认类型
    pub delayed_acknowledgment_type: Option<String>,
}

/// ERR 段（错误）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrSegment {
    /// ERR-1: 错误代码和位置
    pub error_code_and_location: Vec<ErrorCodeAndLocation>,
    /// ERR-2: 错误位置
    pub error_location: Option<String>,
    /// ERR-3: HL7 错误代码
    pub hl7_error_code: Option<String>,
    /// ERR-4: 严重性
    pub severity: Option<String>,
    /// ERR-5: 应用错误代码
    pub application_error_code: Option<String>,
    /// ERR-7: 诊断信息
    pub diagnostic_information: Option<String>,
    /// ERR-8: 用户消息
    pub user_message: Option<String>,
}

/// 错误代码和位置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorCodeAndLocation {
    /// 段 ID
    pub segment_id: Option<String>,
    /// 段序列
    pub segment_sequence: Option<u32>,
    /// 字段位置
    pub field_position: Option<u32>,
    /// 错误代码
    pub error_code: Option<String>,
}

/// ACK 构建器
pub struct AckBuilder {
    original_msh: Option<MshSegment>,
    ack_code: AckCode,
    text_message: Option<String>,
    error: Option<ErrSegment>,
}

impl AckBuilder {
    pub fn new(ack_code: AckCode) -> Self {
        Self {
            original_msh: None,
            ack_code,
            text_message: None,
            error: None,
        }
    }

    /// 从原始消息构建
    pub fn from_message(msg: &Message, ack_code: AckCode) -> Self {
        let msh = MshSegment {
            field_separator: '|',
            encoding_characters: "^~\\&".to_string(),
            sending_application: msg
                .metadata
                .receiving_application
                .clone()
                .unwrap_or_default(),
            sending_facility: msg.metadata.receiving_facility.clone().unwrap_or_default(),
            receiving_application: msg.metadata.sending_application.clone().unwrap_or_default(),
            receiving_facility: msg.metadata.sending_facility.clone().unwrap_or_default(),
            message_datetime: Timestamp::now().time,
            security: None,
            message_type: MessageType::new("ACK", ""),
            message_control_id: ulid::Ulid::new().to_string(),
            processing_id: msg
                .metadata
                .processing_id
                .clone()
                .unwrap_or_else(|| "P".to_string()),
            version_id: "2.5.1".to_string(),
        };

        Self {
            original_msh: Some(msh),
            ack_code,
            text_message: None,
            error: None,
        }
    }

    /// 设置文本消息
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text_message = Some(text.into());
        self
    }

    /// 设置错误信息
    pub fn with_error(mut self, error: ErrSegment) -> Self {
        self.error = Some(error);
        self
    }

    /// 构建 ACK 消息
    pub fn build(self, original_control_id: &str) -> AckMessage {
        let msh = self.original_msh.unwrap_or_else(|| MshSegment {
            field_separator: '|',
            encoding_characters: "^~\\&".to_string(),
            sending_application: "HSB".to_string(),
            sending_facility: String::new(),
            receiving_application: String::new(),
            receiving_facility: String::new(),
            message_datetime: Timestamp::now().time,
            security: None,
            message_type: MessageType::new("ACK", ""),
            message_control_id: ulid::Ulid::new().to_string(),
            processing_id: "P".to_string(),
            version_id: "2.5.1".to_string(),
        });

        let msa = MsaSegment {
            acknowledgment_code: self.ack_code.as_str().to_string(),
            message_control_id: original_control_id.to_string(),
            text_message: self.text_message,
            expected_sequence_number: None,
            delayed_acknowledgment_type: None,
        };

        AckMessage {
            msh,
            msa,
            err: self.error,
        }
    }

    /// 构建并序列化为 HL7 字符串
    pub fn build_string(self, original_control_id: &str) -> String {
        let ack = self.build(original_control_id);

        let mut segments = Vec::new();

        // MSH 段
        segments.push(format!(
            "MSH|^~\\&|{}|{}|{}|{}|{}||ACK|{}|{}|{}",
            ack.msh.sending_application,
            ack.msh.sending_facility,
            ack.msh.receiving_application,
            ack.msh.receiving_facility,
            ack.msh.message_datetime,
            ack.msh.message_control_id,
            ack.msh.processing_id,
            ack.msh.version_id
        ));

        // MSA 段
        segments.push(format!(
            "MSA|{}|{}|{}",
            ack.msa.acknowledgment_code,
            ack.msa.message_control_id,
            ack.msa.text_message.as_deref().unwrap_or("")
        ));

        // ERR 段（如果有错误）
        if let Some(err) = &ack.err {
            segments.push(format!(
                "ERR||{}|{}|{}",
                err.hl7_error_code.as_deref().unwrap_or(""),
                err.severity.as_deref().unwrap_or("E"),
                err.user_message.as_deref().unwrap_or("")
            ));
        }

        segments.join(&SEGMENT_SEPARATOR.to_string())
    }

    /// 构建并序列化为带 MLLP 包装的字节
    pub fn build_bytes(self, original_control_id: &str) -> Bytes {
        let content = self.build_string(original_control_id);
        let mut result = Vec::with_capacity(content.len() + 3);
        result.push(MLLP_START_BLOCK);
        result.extend_from_slice(content.as_bytes());
        result.push(MLLP_END_BLOCK);
        result.push(MLLP_CARRIAGE_RETURN);
        Bytes::from(result)
    }
}

/// 快速创建成功 ACK
pub fn ack_success(msg: &Message) -> Bytes {
    let control_id = msg.metadata.message_control_id.as_deref().unwrap_or("");
    AckBuilder::from_message(msg, AckCode::AA)
        .with_text("Message accepted")
        .build_bytes(control_id)
}

/// 快速创建失败 ACK
pub fn ack_error(msg: &Message, error_message: &str) -> Bytes {
    let control_id = msg.metadata.message_control_id.as_deref().unwrap_or("");
    AckBuilder::from_message(msg, AckCode::AE)
        .with_text(error_message)
        .with_error(ErrSegment {
            severity: Some("E".to_string()),
            user_message: Some(error_message.to_string()),
            ..Default::default()
        })
        .build_bytes(control_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_builder() {
        let ack_str = AckBuilder::new(AckCode::AA)
            .with_text("Message processed")
            .build_string("MSG00001");

        assert!(ack_str.contains("MSA|AA|MSG00001"));
        assert!(ack_str.contains("Message processed"));
    }

    #[test]
    fn test_ack_code() {
        assert!(AckCode::AA.is_success());
        assert!(AckCode::CA.is_success());
        assert!(!AckCode::AE.is_success());
        assert!(!AckCode::AR.is_success());
    }
}
