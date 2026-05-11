//! TCP 客户端

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{TcpTransportConfig, mllp};

/// TCP 客户端
pub struct TcpClient {
    config: TcpTransportConfig,
}

impl TcpClient {
    pub fn new(config: TcpTransportConfig) -> Self {
        Self { config }
    }

    /// 发送消息并等待响应
    pub async fn send(&self, message: Bytes) -> HsbResult<Bytes> {
        self.send_with_timeout(message, Duration::from_secs(self.config.timeout_secs))
            .await
    }

    /// 发送消息并等待响应（带超时）
    pub async fn send_with_timeout(&self, message: Bytes, timeout: Duration) -> HsbResult<Bytes> {
        let addr = format!("{}:{}", self.config.host, self.config.port);

        // 连接
        let mut stream = tokio::time::timeout(
            Duration::from_secs(self.config.connect_timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| HsbError::TimeoutError {
            operation: "TCP connect".to_string(),
            timeout_ms: self.config.connect_timeout_secs * 1000,
        })?
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to connect to {}: {}", addr, e),
        })?;

        // 设置选项
        if self.config.no_delay {
            stream.set_nodelay(true).ok();
        }

        // 准备数据
        let data = if self.config.use_mllp {
            mllp::wrap_mllp(&message)
        } else {
            message.to_vec()
        };

        // 发送
        tokio::time::timeout(timeout, stream.write_all(&data))
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "TCP write".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })?
            .map_err(|e| HsbError::TransportError {
                message: format!("Write failed: {}", e),
            })?;

        // 接收响应
        let mut buffer = vec![0u8; self.config.buffer_size];
        let n = tokio::time::timeout(timeout, stream.read(&mut buffer))
            .await
            .map_err(|_| HsbError::TimeoutError {
                operation: "TCP read".to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })?
            .map_err(|e| HsbError::TransportError {
                message: format!("Read failed: {}", e),
            })?;

        buffer.truncate(n);

        // 解包响应
        let response = if self.config.use_mllp {
            mllp::unwrap_mllp(&buffer)?
        } else {
            buffer
        };

        Ok(Bytes::from(response))
    }

    /// 发送消息（不等待响应）
    pub async fn send_no_response(&self, message: Bytes) -> HsbResult<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);

        let mut stream = tokio::time::timeout(
            Duration::from_secs(self.config.connect_timeout_secs),
            TcpStream::connect(&addr),
        )
        .await
        .map_err(|_| HsbError::TimeoutError {
            operation: "TCP connect".to_string(),
            timeout_ms: self.config.connect_timeout_secs * 1000,
        })?
        .map_err(|e| HsbError::TransportError {
            message: format!("Failed to connect to {}: {}", addr, e),
        })?;

        let data = if self.config.use_mllp {
            mllp::wrap_mllp(&message)
        } else {
            message.to_vec()
        };

        stream
            .write_all(&data)
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Write failed: {}", e),
            })?;

        Ok(())
    }
}
