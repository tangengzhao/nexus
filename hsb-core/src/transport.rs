//! HSB 传输层基础定义
//!
//! 定义传输层的核心 trait 和通用类型。

use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbResult, SystemId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// 传输层 Trait
#[async_trait]
pub trait Transport: Send + Sync {
    /// 获取传输类型名称
    fn transport_type(&self) -> TransportType;

    /// 获取传输名称
    fn name(&self) -> &str;

    /// 发送消息
    async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse>;

    /// 发送消息（带超时）
    async fn send_with_timeout(
        &self,
        request: TransportRequest,
        timeout: Duration,
    ) -> HsbResult<TransportResponse>;

    /// 健康检查
    async fn health_check(&self) -> HsbResult<HealthStatus>;

    /// 获取连接统计
    fn stats(&self) -> TransportStats;
}

/// 可连接的传输（需要持续连接）
#[async_trait]
pub trait ConnectableTransport: Transport {
    /// 建立连接
    async fn connect(&self) -> HsbResult<()>;

    /// 断开连接
    async fn disconnect(&self) -> HsbResult<()>;

    /// 重新连接
    async fn reconnect(&self) -> HsbResult<()>;

    /// 是否已连接
    fn is_connected(&self) -> bool;
}

/// 可监听的传输（服务端）
#[async_trait]
pub trait ListenableTransport: Transport {
    /// 开始监听
    async fn start_listening(&self) -> HsbResult<()>;

    /// 停止监听
    async fn stop_listening(&self) -> HsbResult<()>;

    /// 是否正在监听
    fn is_listening(&self) -> bool;

    /// 设置消息处理器
    fn set_handler(&self, handler: Arc<dyn MessageHandler>);
}

/// 消息处理器
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理接收到的消息
    async fn handle(&self, data: Bytes, context: ConnectionContext) -> HsbResult<Bytes>;
}

/// 传输类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportType {
    Http,
    Https,
    TcpMllp,
    Grpc,
    Mq,
    Nats,
    WebSocket,
}

impl TransportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Https => "HTTPS",
            Self::TcpMllp => "TCP/MLLP",
            Self::Grpc => "gRPC",
            Self::Mq => "MQ",
            Self::Nats => "NATS",
            Self::WebSocket => "WebSocket",
        }
    }
}

/// 传输请求
#[derive(Debug, Clone)]
pub struct TransportRequest {
    /// 目标地址
    pub target: String,
    /// 请求体
    pub body: Bytes,
    /// 请求头
    pub headers: HashMap<String, String>,
    /// 超时时间
    pub timeout: Option<Duration>,
    /// 请求元数据
    pub metadata: RequestMetadata,
}

impl TransportRequest {
    pub fn new(target: &str, body: Bytes) -> Self {
        Self {
            target: target.to_string(),
            body,
            headers: HashMap::new(),
            timeout: None,
            metadata: RequestMetadata::default(),
        }
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_trace_id(mut self, trace_id: &str) -> Self {
        self.metadata.trace_id = Some(trace_id.to_string());
        self
    }
}

/// 请求元数据
#[derive(Debug, Clone, Default)]
pub struct RequestMetadata {
    /// 追踪 ID
    pub trace_id: Option<String>,
    /// 消息 ID
    pub message_id: Option<String>,
    /// 源系统
    pub source_system: Option<SystemId>,
    /// 目标系统
    pub target_system: Option<SystemId>,
    /// 优先级
    pub priority: Option<u8>,
    /// 重试次数
    pub retry_count: u32,
}

/// 传输响应
#[derive(Debug, Clone)]
pub struct TransportResponse {
    /// 状态码
    pub status_code: u16,
    /// 响应体
    pub body: Bytes,
    /// 响应头
    pub headers: HashMap<String, String>,
    /// 响应时间
    pub duration: Duration,
    /// 响应元数据
    pub metadata: ResponseMetadata,
}

impl TransportResponse {
    pub fn success(body: Bytes, duration: Duration) -> Self {
        Self {
            status_code: 200,
            body,
            headers: HashMap::new(),
            duration,
            metadata: ResponseMetadata::default(),
        }
    }

    pub fn error(status_code: u16, message: &str, duration: Duration) -> Self {
        Self {
            status_code,
            body: Bytes::from(message.to_string()),
            headers: HashMap::new(),
            duration,
            metadata: ResponseMetadata::default(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }
}

/// 响应元数据
#[derive(Debug, Clone, Default)]
pub struct ResponseMetadata {
    /// 服务器版本
    pub server_version: Option<String>,
    /// 内容类型
    pub content_type: Option<String>,
    /// 编码
    pub encoding: Option<String>,
}

/// 连接上下文
#[derive(Debug, Clone)]
pub struct ConnectionContext {
    /// 远程地址
    pub remote_addr: String,
    /// 本地地址
    pub local_addr: String,
    /// 连接 ID
    pub connection_id: String,
    /// TLS 信息
    pub tls_info: Option<TlsInfo>,
}

/// TLS 信息
#[derive(Debug, Clone)]
pub struct TlsInfo {
    /// 协议版本
    pub version: String,
    /// 密码套件
    pub cipher_suite: String,
    /// 客户端证书 CN
    pub client_cn: Option<String>,
}

/// 健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// 是否健康
    pub healthy: bool,
    /// 状态消息
    pub message: String,
    /// 最后检查时间
    pub last_check: chrono::DateTime<chrono::Utc>,
    /// 详细信息
    pub details: HashMap<String, String>,
}

impl HealthStatus {
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            message: "OK".to_string(),
            last_check: chrono::Utc::now(),
            details: HashMap::new(),
        }
    }

    pub fn unhealthy(message: &str) -> Self {
        Self {
            healthy: false,
            message: message.to_string(),
            last_check: chrono::Utc::now(),
            details: HashMap::new(),
        }
    }

    pub fn with_detail(mut self, key: &str, value: &str) -> Self {
        self.details.insert(key.to_string(), value.to_string());
        self
    }
}

/// 传输统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportStats {
    /// 发送消息数
    pub messages_sent: u64,
    /// 接收消息数
    pub messages_received: u64,
    /// 发送字节数
    pub bytes_sent: u64,
    /// 接收字节数
    pub bytes_received: u64,
    /// 错误数
    pub errors: u64,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
    /// 最大响应时间（毫秒）
    pub max_response_time_ms: u64,
    /// 当前连接数
    pub active_connections: u32,
}

/// 传输注册表
pub struct TransportRegistry {
    transports: HashMap<String, Arc<dyn Transport>>,
}

impl TransportRegistry {
    pub fn new() -> Self {
        Self {
            transports: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, transport: Arc<dyn Transport>) {
        self.transports.insert(name.to_string(), transport);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Transport>> {
        self.transports.get(name).cloned()
    }

    pub fn remove(&mut self, name: &str) -> Option<Arc<dyn Transport>> {
        self.transports.remove(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.transports.keys().cloned().collect()
    }
}

impl Default for TransportRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 连接池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolConfig {
    /// 最小连接数
    pub min_connections: u32,
    /// 最大连接数
    pub max_connections: u32,
    /// 连接超时（秒）
    pub connection_timeout_secs: u64,
    /// 空闲超时（秒）
    pub idle_timeout_secs: u64,
    /// 最大生命周期（秒）
    pub max_lifetime_secs: Option<u64>,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            connection_timeout_secs: 30,
            idle_timeout_secs: 300,
            max_lifetime_secs: Some(3600),
        }
    }
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 退避乘数
    pub backoff_multiplier: f64,
    /// 是否启用抖动
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_http_target() -> String {
        std::env::var("HSB_TEST_HTTP_TARGET")
            .unwrap_or_else(|_| "http://gateway.internal:8080".to_string())
    }

    #[test]
    fn test_transport_request() {
        let target = test_http_target();
        let request = TransportRequest::new(&target, Bytes::from("test"))
            .with_header("Content-Type", "application/json")
            .with_timeout(Duration::from_secs(30));

        assert_eq!(request.target, target);
        assert_eq!(
            request.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(request.timeout, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_transport_response() {
        let response = TransportResponse::success(Bytes::from("OK"), Duration::from_millis(100));
        assert!(response.is_success());
        assert_eq!(response.status_code, 200);
    }
}
