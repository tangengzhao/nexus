//! MLLP 协议实现
//!
//! MLLP (Minimal Lower Layer Protocol) 是 HL7 v2.x 的标准传输协议。
//!
//! 帧格式：
//! - 起始块: 0x0B (VT, Vertical Tab)
//! - 数据: HL7 消息
//! - 结束块: 0x1C (FS, File Separator)
//! - 回车: 0x0D (CR, Carriage Return)

use hsb_common::{HsbError, HsbResult};

/// MLLP 起始字节
pub const MLLP_START: u8 = 0x0B;
/// MLLP 结束字节
pub const MLLP_END: u8 = 0x1C;
/// MLLP 回车字节
pub const MLLP_CR: u8 = 0x0D;

/// MLLP 帧
#[derive(Debug, Clone)]
pub struct MllpFrame {
    /// 消息数据
    pub data: Vec<u8>,
}

impl MllpFrame {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// 编码为 MLLP 格式
    pub fn encode(&self) -> Vec<u8> {
        wrap_mllp(&self.data)
    }

    /// 从 MLLP 格式解码
    pub fn decode(raw: &[u8]) -> HsbResult<Self> {
        let data = unwrap_mllp(raw)?;
        Ok(Self { data })
    }
}

/// 将数据包装为 MLLP 格式
pub fn wrap_mllp(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len() + 3);
    result.push(MLLP_START);
    result.extend_from_slice(data);
    result.push(MLLP_END);
    result.push(MLLP_CR);
    result
}

/// 从 MLLP 格式解包数据
pub fn unwrap_mllp(data: &[u8]) -> HsbResult<Vec<u8>> {
    if data.len() < 3 {
        return Err(HsbError::ParseError {
            message: "MLLP frame too short".to_string(),
        });
    }

    // 检查起始字节
    if data[0] != MLLP_START {
        return Err(HsbError::ParseError {
            message: format!("Invalid MLLP start byte: 0x{:02X}", data[0]),
        });
    }

    // 查找结束位置
    let end_pos = data.iter().position(|&b| b == MLLP_END);

    match end_pos {
        Some(pos) if pos > 0 => Ok(data[1..pos].to_vec()),
        Some(_) => Err(HsbError::ParseError {
            message: "Empty MLLP frame".to_string(),
        }),
        None => Err(HsbError::ParseError {
            message: "Missing MLLP end byte".to_string(),
        }),
    }
}

/// MLLP 编解码器
pub struct MllpCodec {
    buffer: Vec<u8>,
    in_message: bool,
}

impl MllpCodec {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            in_message: false,
        }
    }

    /// 重置编解码器状态
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.in_message = false;
    }

    /// 输入数据并尝试提取完整消息
    pub fn input(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        let mut messages = Vec::new();

        for &byte in data {
            match byte {
                MLLP_START => {
                    // 开始新消息
                    self.buffer.clear();
                    self.in_message = true;
                }
                MLLP_END if self.in_message => {
                    // 消息结束
                    messages.push(self.buffer.clone());
                    self.buffer.clear();
                    self.in_message = false;
                }
                MLLP_CR if !self.in_message => {
                    // 忽略消息外的 CR
                }
                _ if self.in_message => {
                    // 消息内容
                    self.buffer.push(byte);
                }
                _ => {
                    // 忽略消息外的其他字节
                }
            }
        }

        messages
    }

    /// 编码消息
    pub fn encode(&self, message: &[u8]) -> Vec<u8> {
        wrap_mllp(message)
    }
}

impl Default for MllpCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_unwrap() {
        let original = b"MSH|^~\\&|TEST|";
        let wrapped = wrap_mllp(original);

        assert_eq!(wrapped[0], MLLP_START);
        assert_eq!(wrapped[wrapped.len() - 2], MLLP_END);
        assert_eq!(wrapped[wrapped.len() - 1], MLLP_CR);

        let unwrapped = unwrap_mllp(&wrapped).expect("Should unwrap");
        assert_eq!(unwrapped, original);
    }

    #[test]
    fn test_codec() {
        let mut codec = MllpCodec::new();

        let msg1 = b"MSG1";
        let msg2 = b"MSG2";

        let frame1 = wrap_mllp(msg1);
        let frame2 = wrap_mllp(msg2);

        // 模拟分段接收
        let mut combined = Vec::new();
        combined.extend_from_slice(&frame1);
        combined.extend_from_slice(&frame2);

        let messages = codec.input(&combined);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], msg1.to_vec());
        assert_eq!(messages[1], msg2.to_vec());
    }

    #[test]
    fn test_invalid_frame() {
        let invalid = b"no start byte";
        let result = unwrap_mllp(invalid);
        assert!(result.is_err());
    }
}
