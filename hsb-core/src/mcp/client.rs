//! MCP 客户端：总线作为客户端，通过 Streamable HTTP 调用下游 MCP Server

use crate::engine::{EndpointInfo, EndpointRegistry};
use crate::transport::{TransportRegistry, TransportRequest};
use bytes::Bytes;
use hsb_common::ProtocolType;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

/// 总线支持的 MCP 协议版本（从新到旧）
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// 默认调用超时（秒）
pub const DEFAULT_MCP_TIMEOUT_SECS: u64 = 30;

const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const HEADER_PROTOCOL_VERSION: &str = "MCP-Protocol-Version";
const MAX_LIST_PAGES: usize = 20;

/// JSON-RPC 错误码
pub mod codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    /// 下游 MCP 服务不可用（网络、认证、协议错误等）
    pub const UPSTREAM_ERROR: i64 = -32000;
}

/// MCP 调用错误
#[derive(Debug, Clone)]
pub struct McpError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl McpError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(codes::UPSTREAM_ERROR, message)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(codes::INVALID_PARAMS, message)
    }

    /// 是否为下游不可用（相对于下游明确返回的 JSON-RPC 错误）
    pub fn is_upstream(&self) -> bool {
        self.code == codes::UPSTREAM_ERROR
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for McpError {}

#[derive(Debug)]
struct McpSession {
    /// 地址与认证头的指纹，端点配置变化后需要重新握手
    fingerprint: String,
    session_id: Option<String>,
    protocol_version: String,
}

/// MCP 客户端
pub struct McpClient {
    transports: Arc<RwLock<TransportRegistry>>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    sessions: RwLock<HashMap<String, Arc<McpSession>>>,
    init_lock: Mutex<()>,
    next_id: AtomicU64,
}

impl McpClient {
    pub fn new(
        transports: Arc<RwLock<TransportRegistry>>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
    ) -> Self {
        Self {
            transports,
            endpoints,
            sessions: RwLock::new(HashMap::new()),
            init_lock: Mutex::new(()),
            next_id: AtomicU64::new(1),
        }
    }

    /// 取得已启用的 MCP 端点
    pub async fn endpoint(&self, endpoint_id: &str) -> Result<EndpointInfo, McpError> {
        let endpoints = self.endpoints.read().await;
        match endpoints.get(endpoint_id) {
            Some(info) if info.protocol == ProtocolType::Mcp && info.enabled => Ok(info.clone()),
            Some(info) if info.protocol != ProtocolType::Mcp => Err(McpError::invalid_params(
                format!("端点 {endpoint_id} 不是 MCP 类型"),
            )),
            Some(_) => Err(McpError::upstream(format!("端点 {endpoint_id} 已停用"))),
            None => Err(McpError::invalid_params(format!(
                "端点 {endpoint_id} 不存在"
            ))),
        }
    }

    /// 列出下游 MCP Server 的全部工具（自动翻页）
    pub async fn list_tools(&self, endpoint: &EndpointInfo) -> Result<Vec<Value>, McpError> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_LIST_PAGES {
            let params = match &cursor {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            let result = self.request(endpoint, "tools/list", params).await?;
            if let Some(items) = result.get("tools").and_then(Value::as_array) {
                tools.extend(items.iter().cloned());
            }
            cursor = result
                .get("nextCursor")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(tools)
    }

    /// 调用下游工具，返回 `tools/call` 的 result
    pub async fn call_tool(
        &self,
        endpoint: &EndpointInfo,
        tool: &str,
        arguments: Value,
    ) -> Result<Value, McpError> {
        self.request(
            endpoint,
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
        .await
    }

    /// 清除端点会话（端点被修改或删除时调用）
    pub async fn invalidate(&self, endpoint_id: &str) {
        self.sessions.write().await.remove(endpoint_id);
    }

    /// 发送 JSON-RPC 请求；会话失效（404）时自动重新握手并重试一次
    pub async fn request(
        &self,
        endpoint: &EndpointInfo,
        method: &str,
        params: Value,
    ) -> Result<Value, McpError> {
        for attempt in 0..2 {
            let session = self.ensure_session(endpoint).await?;
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });

            let response = self.post(endpoint, Some(&session), &body).await?;
            if response.status == 404 && session.session_id.is_some() && attempt == 0 {
                self.invalidate(&endpoint.id).await;
                continue;
            }
            if !(200..300).contains(&response.status) {
                return Err(McpError::upstream(format!(
                    "MCP 服务 {} 返回 HTTP {}: {}",
                    endpoint.name,
                    response.status,
                    truncate(&String::from_utf8_lossy(&response.body), 200)
                )));
            }
            let message =
                parse_rpc_response(&response.content_type, &response.body, id).map_err(|e| {
                    McpError::upstream(format!("MCP 服务 {} 响应无效: {e}", endpoint.name))
                })?;
            return rpc_result(message);
        }
        Err(McpError::upstream(format!(
            "MCP 服务 {} 会话重建失败",
            endpoint.name
        )))
    }

    async fn ensure_session(&self, endpoint: &EndpointInfo) -> Result<Arc<McpSession>, McpError> {
        let fingerprint = fingerprint(endpoint);
        if let Some(session) = self.sessions.read().await.get(&endpoint.id)
            && session.fingerprint == fingerprint
        {
            return Ok(session.clone());
        }

        let _guard = self.init_lock.lock().await;
        if let Some(session) = self.sessions.read().await.get(&endpoint.id)
            && session.fingerprint == fingerprint
        {
            return Ok(session.clone());
        }

        let session = Arc::new(self.initialize(endpoint, fingerprint).await?);
        self.sessions
            .write()
            .await
            .insert(endpoint.id.clone(), session.clone());
        Ok(session)
    }

    /// MCP 握手：initialize + notifications/initialized
    async fn initialize(
        &self,
        endpoint: &EndpointInfo,
        fingerprint: String,
    ) -> Result<McpSession, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": SUPPORTED_PROTOCOL_VERSIONS[0],
                "capabilities": {},
                "clientInfo": { "name": "nexus-hsb", "version": env!("CARGO_PKG_VERSION") },
            },
        });
        let response = self.post(endpoint, None, &body).await?;
        if !(200..300).contains(&response.status) {
            return Err(McpError::upstream(format!(
                "MCP 服务 {} 握手失败，HTTP {}: {}",
                endpoint.name,
                response.status,
                truncate(&String::from_utf8_lossy(&response.body), 200)
            )));
        }
        let message =
            parse_rpc_response(&response.content_type, &response.body, id).map_err(|e| {
                McpError::upstream(format!("MCP 服务 {} 握手响应无效: {e}", endpoint.name))
            })?;
        let result = rpc_result(message)?;

        let protocol_version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[2])
            .to_string();
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&protocol_version.as_str()) {
            return Err(McpError::upstream(format!(
                "MCP 服务 {} 使用了不支持的协议版本 {}",
                endpoint.name, protocol_version
            )));
        }

        let session = McpSession {
            fingerprint,
            session_id: response.session_id,
            protocol_version,
        };

        // 通知服务端握手完成；部分服务不接受该通知，失败不影响后续调用
        let notice = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        if let Err(e) = self.post(endpoint, Some(&session), &notice).await {
            tracing::debug!(
                "MCP initialized notification to {} failed: {}",
                endpoint.id,
                e
            );
        }
        Ok(session)
    }

    async fn post(
        &self,
        endpoint: &EndpointInfo,
        session: Option<&McpSession>,
        body: &Value,
    ) -> Result<RawResponse, McpError> {
        let transport = self
            .transports
            .read()
            .await
            .get("http")
            .ok_or_else(|| McpError::upstream("总线未注册 http 传输"))?;

        let timeout = endpoint
            .properties
            .get("mcp_timeout_secs")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MCP_TIMEOUT_SECS);

        let payload = serde_json::to_vec(body)
            .map_err(|e| McpError::new(codes::INTERNAL_ERROR, e.to_string()))?;
        let mut request = TransportRequest::new(&endpoint.address, Bytes::from(payload))
            .with_headers(endpoint.headers.clone())
            .with_header("Content-Type", "application/json")
            .with_header("Accept", "application/json, text/event-stream")
            .with_timeout(Duration::from_secs(timeout));
        if let Some(session) = session {
            request = request.with_header(HEADER_PROTOCOL_VERSION, &session.protocol_version);
            if let Some(session_id) = &session.session_id {
                request = request.with_header(HEADER_SESSION_ID, session_id);
            }
        }

        let response = transport
            .send(request)
            .await
            .map_err(|e| McpError::upstream(format!("无法连接 MCP 服务 {}: {e}", endpoint.name)))?;

        let header = |name: &str| {
            response
                .headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.clone())
        };
        Ok(RawResponse {
            status: response.status_code,
            content_type: header("content-type").unwrap_or_default(),
            session_id: header(HEADER_SESSION_ID),
            body: response.body,
        })
    }
}

struct RawResponse {
    status: u16,
    content_type: String,
    session_id: Option<String>,
    body: Bytes,
}

fn fingerprint(endpoint: &EndpointInfo) -> String {
    let mut headers: Vec<_> = endpoint.headers.iter().collect();
    headers.sort();
    format!("{}|{:?}", endpoint.address, headers)
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let head: String = text.chars().take(max).collect();
        format!("{head}…")
    }
}

/// 从响应体（JSON 或 SSE）中取出与请求 id 对应的 JSON-RPC 响应
pub(crate) fn parse_rpc_response(
    content_type: &str,
    body: &[u8],
    expected_id: u64,
) -> Result<Value, String> {
    let text = std::str::from_utf8(body).map_err(|e| e.to_string())?;
    let candidates: Vec<Value> = if content_type
        .to_ascii_lowercase()
        .contains("text/event-stream")
    {
        parse_sse_messages(text)
    } else {
        vec![serde_json::from_str(text).map_err(|e| e.to_string())?]
    };

    let matches_id = |message: &Value| {
        message.get("id").and_then(Value::as_u64) == Some(expected_id)
            && (message.get("result").is_some() || message.get("error").is_some())
    };
    for candidate in candidates {
        match candidate {
            Value::Array(items) => {
                if let Some(found) = items.into_iter().find(matches_id) {
                    return Ok(found);
                }
            }
            message if matches_id(&message) => return Ok(message),
            _ => {}
        }
    }
    Err("响应中没有对应请求的结果".to_string())
}

fn parse_sse_messages(text: &str) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut data_lines: Vec<&str> = Vec::new();
    let mut flush = |data_lines: &mut Vec<&str>| {
        if !data_lines.is_empty() {
            if let Ok(value) = serde_json::from_str::<Value>(&data_lines.join("\n")) {
                messages.push(value);
            }
            data_lines.clear();
        }
    };
    for line in text.lines() {
        if line.is_empty() {
            flush(&mut data_lines);
        } else if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.strip_prefix(' ').unwrap_or(data));
        }
    }
    flush(&mut data_lines);
    messages
}

fn rpc_result(message: Value) -> Result<Value, McpError> {
    if let Some(error) = message.get("error") {
        return Err(McpError {
            code: error
                .get("code")
                .and_then(Value::as_i64)
                .unwrap_or(codes::INTERNAL_ERROR),
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("MCP error")
                .to_string(),
            data: error.get("data").cloned(),
        });
    }
    Ok(message.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_and_matches_id() {
        let body = br#"{"jsonrpc":"2.0","id":7,"result":{"ok":true}}"#;
        let message = parse_rpc_response("application/json", body, 7).unwrap();
        assert_eq!(rpc_result(message).unwrap()["ok"], true);
        assert!(parse_rpc_response("application/json", body, 8).is_err());
    }

    #[test]
    fn parses_sse_stream_ignoring_notifications() {
        let body = b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}\n\nevent: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"tools\":[]}}\n\n";
        let message = parse_rpc_response("text/event-stream", body, 3).unwrap();
        assert!(rpc_result(message).unwrap()["tools"].is_array());
    }

    #[test]
    fn maps_jsonrpc_error() {
        let message = json!({"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"bad"}});
        let error = rpc_result(message).unwrap_err();
        assert_eq!(error.code, codes::INVALID_PARAMS);
        assert!(!error.is_upstream());
    }
}
