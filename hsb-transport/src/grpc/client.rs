//! gRPC 客户端

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use std::collections::HashMap;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::info;

use crate::GrpcTransportConfig;

/// 通用 gRPC 客户端
pub struct GrpcClient {
    channel: Channel,
    config: GrpcTransportConfig,
}

impl GrpcClient {
    /// 创建新客户端
    pub async fn new(config: GrpcTransportConfig) -> HsbResult<Self> {
        let endpoint = tonic::transport::Endpoint::from_shared(config.endpoint.clone())
            .map_err(|e| HsbError::ConfigError {
                message: format!("Invalid gRPC endpoint: {}", e),
            })?
            .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
            .timeout(Duration::from_secs(config.request_timeout_secs));

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: config.endpoint.clone(),
                message: e.to_string(),
            })?;

        info!("gRPC client connected to {}", config.endpoint);

        Ok(Self { channel, config })
    }

    /// 获取底层 channel
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// 克隆 channel
    pub fn clone_channel(&self) -> Channel {
        self.channel.clone()
    }

    /// 重新连接
    pub async fn reconnect(&mut self) -> HsbResult<()> {
        let new_client = Self::new(self.config.clone()).await?;
        self.channel = new_client.channel;
        Ok(())
    }
}

/// gRPC 请求构建器
pub struct GrpcRequestBuilder {
    service: String,
    method: String,
    body: Bytes,
    metadata: HashMap<String, String>,
    timeout: Option<Duration>,
}

impl GrpcRequestBuilder {
    pub fn new(service: &str, method: &str) -> Self {
        Self {
            service: service.to_string(),
            method: method.to_string(),
            body: Bytes::new(),
            metadata: HashMap::new(),
            timeout: None,
        }
    }

    pub fn body(mut self, body: Bytes) -> Self {
        self.body = body;
        self
    }

    pub fn metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// 构建 gRPC 路径
    pub fn path(&self) -> String {
        format!("/{}/{}", self.service, self.method)
    }

    /// 构建 tonic 请求
    pub fn build<T: prost::Message + Default>(self) -> HsbResult<tonic::Request<T>> {
        let message = T::decode(self.body).map_err(|e| HsbError::ParseError {
            message: format!("Failed to decode protobuf message: {}", e),
        })?;

        let mut request = tonic::Request::new(message);

        // 添加元数据
        for (key, value) in self.metadata {
            if let Ok(meta_key) =
                tonic::metadata::MetadataKey::<tonic::metadata::Ascii>::from_bytes(key.as_bytes())
            {
                if let Ok(meta_value) =
                    tonic::metadata::MetadataValue::<tonic::metadata::Ascii>::try_from(&value)
                {
                    request.metadata_mut().insert(meta_key, meta_value);
                }
            }
        }

        // 设置超时
        if let Some(timeout) = self.timeout {
            request.set_timeout(timeout);
        }

        Ok(request)
    }
}

/// gRPC 响应
#[derive(Debug, Clone)]
pub struct GrpcResponse {
    pub status: tonic::Status,
    pub body: Bytes,
    pub metadata: HashMap<String, String>,
}

impl GrpcResponse {
    pub fn success(body: Bytes) -> Self {
        Self {
            status: tonic::Status::ok("success"),
            body,
            metadata: HashMap::new(),
        }
    }

    pub fn error(status: tonic::Status) -> Self {
        Self {
            status,
            body: Bytes::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status.code() == tonic::Code::Ok
    }
}
