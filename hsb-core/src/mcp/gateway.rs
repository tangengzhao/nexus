//! MCP 网关：总线作为 MCP Server 对外服务智能体，聚合多个下游 MCP Server 的工具

use super::client::{McpClient, McpError, SUPPORTED_PROTOCOL_VERSIONS, codes};
use crate::engine::{EndpointInfo, EndpointRegistry};
use chrono::{DateTime, Utc};
use hsb_common::ProtocolType;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 暴露给智能体的工具名分隔符：`{前缀}__{工具名}`
pub const TOOL_NAME_SEPARATOR: &str = "__";

/// 允许访问网关的智能体（客户端）
#[derive(Clone, Serialize, Deserialize)]
pub struct McpClientConfig {
    /// 智能体名称（用于审计）
    pub name: String,
    /// 访问令牌（`Authorization: Bearer <token>` 或 `X-API-Key`）
    pub token: String,
    /// 允许访问的 MCP 端点 ID；为空表示全部
    #[serde(default)]
    pub allowed_endpoints: Vec<String>,
}

impl fmt::Debug for McpClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpClientConfig")
            .field("name", &self.name)
            .field("token", &"***")
            .field("allowed_endpoints", &self.allowed_endpoints)
            .finish()
    }
}

/// MCP 网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpGatewayConfig {
    /// 是否启用网关
    pub enabled: bool,
    /// 网关 HTTP 路径
    pub path: String,
    /// 未配置任何智能体令牌时是否允许匿名访问（仅建议在受信网络的开发环境使用）
    pub allow_anonymous: bool,
    /// 工具清单缓存时间（秒）
    pub tool_cache_secs: u64,
    /// 保留的最近调用记录数
    pub call_log_capacity: usize,
    /// 允许访问的智能体
    pub clients: Vec<McpClientConfig>,
}

impl Default for McpGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/mcp".to_string(),
            allow_anonymous: false,
            tool_cache_secs: 30,
            call_log_capacity: 200,
            clients: Vec::new(),
        }
    }
}

/// 调用方身份
#[derive(Debug, Clone)]
pub struct McpCaller {
    pub name: String,
    /// 为空表示可访问全部端点
    pub allowed_endpoints: Vec<String>,
}

impl McpCaller {
    /// 管理界面的调试调用
    pub fn admin() -> Self {
        Self {
            name: "admin-console".to_string(),
            allowed_endpoints: Vec::new(),
        }
    }

    fn can_access(&self, endpoint_id: &str) -> bool {
        self.allowed_endpoints.is_empty()
            || self.allowed_endpoints.iter().any(|id| id == endpoint_id)
    }
}

/// 网关认证失败原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAuthError {
    /// 缺少或错误的令牌
    Unauthorized,
    /// 未配置令牌且不允许匿名访问
    NotConfigured,
}

/// 聚合后的工具信息
#[derive(Debug, Clone, Serialize)]
pub struct McpToolInfo {
    /// 暴露给智能体的名称（带端点前缀）
    pub name: String,
    pub endpoint_id: String,
    pub endpoint_name: String,
    /// 下游 MCP Server 中的原始名称
    pub original_name: String,
    pub description: Option<String>,
    pub input_schema: Value,
}

/// MCP 端点状态
#[derive(Debug, Clone, Serialize)]
pub struct McpEndpointStatus {
    pub endpoint_id: String,
    pub endpoint_name: String,
    pub address: String,
    pub prefix: String,
    pub enabled: bool,
    pub tool_count: usize,
    pub error: Option<String>,
}

/// 工具目录
#[derive(Debug, Clone, Serialize)]
pub struct McpToolCatalog {
    pub tools: Vec<McpToolInfo>,
    pub endpoints: Vec<McpEndpointStatus>,
}

/// 一次工具调用记录
#[derive(Debug, Clone, Serialize)]
pub struct McpCallRecord {
    pub id: String,
    pub time: DateTime<Utc>,
    pub caller: String,
    pub tool: String,
    pub endpoint_id: Option<String>,
    pub success: bool,
    pub duration_ms: u64,
    pub error: Option<String>,
}

struct CachedTools {
    fetched_at: Instant,
    tools: Vec<Value>,
}

/// MCP 网关
pub struct McpGateway {
    client: Arc<McpClient>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    config: McpGatewayConfig,
    tool_cache: RwLock<HashMap<String, CachedTools>>,
    calls: Mutex<VecDeque<McpCallRecord>>,
}

impl McpGateway {
    pub fn new(
        client: Arc<McpClient>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
        config: McpGatewayConfig,
    ) -> Self {
        Self {
            client,
            endpoints,
            config,
            tool_cache: RwLock::new(HashMap::new()),
            calls: Mutex::new(VecDeque::new()),
        }
    }

    pub fn config(&self) -> &McpGatewayConfig {
        &self.config
    }

    pub fn client(&self) -> &Arc<McpClient> {
        &self.client
    }

    /// 校验智能体令牌
    pub fn authenticate(
        &self,
        authorization: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<McpCaller, McpAuthError> {
        if self.config.clients.is_empty() {
            return if self.config.allow_anonymous {
                Ok(McpCaller {
                    name: "anonymous".to_string(),
                    allowed_endpoints: Vec::new(),
                })
            } else {
                Err(McpAuthError::NotConfigured)
            };
        }

        let presented = authorization
            .and_then(|value| {
                let value = value.trim();
                value
                    .strip_prefix("Bearer ")
                    .or_else(|| value.strip_prefix("bearer "))
            })
            .or(api_key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(McpAuthError::Unauthorized)?;

        // 逐个比较且不提前退出，避免通过耗时推断令牌
        let mut matched: Option<&McpClientConfig> = None;
        for client in &self.config.clients {
            if constant_time_eq(client.token.as_bytes(), presented.as_bytes()) {
                matched = Some(client);
            }
        }
        matched
            .map(|client| McpCaller {
                name: client.name.clone(),
                allowed_endpoints: client.allowed_endpoints.clone(),
            })
            .ok_or(McpAuthError::Unauthorized)
    }

    /// 聚合所有 MCP 端点的工具目录；`refresh` 为真时忽略缓存
    pub async fn catalog(&self, refresh: bool) -> McpToolCatalog {
        let mut endpoints: Vec<EndpointInfo> = {
            let registry = self.endpoints.read().await;
            registry
                .by_protocol(ProtocolType::Mcp)
                .into_iter()
                .cloned()
                .collect()
        };
        endpoints.sort_by(|a, b| a.id.cmp(&b.id));

        if refresh {
            self.tool_cache.write().await.clear();
        }

        let mut tasks = tokio::task::JoinSet::new();
        let mut seen_prefixes: HashSet<String> = HashSet::new();
        let mut statuses: Vec<McpEndpointStatus> = Vec::new();

        for endpoint in &endpoints {
            let prefix = endpoint_prefix(endpoint);
            let mut status = McpEndpointStatus {
                endpoint_id: endpoint.id.clone(),
                endpoint_name: endpoint.name.clone(),
                address: endpoint.address.clone(),
                prefix: prefix.clone(),
                enabled: endpoint.enabled,
                tool_count: 0,
                error: None,
            };
            if !endpoint.enabled {
                status.error = Some("端点已停用".to_string());
            } else if !seen_prefixes.insert(prefix.clone()) {
                status.error = Some(format!(
                    "工具前缀 {prefix} 与其他端点冲突，请修改“工具前缀”"
                ));
            } else {
                let gateway_client = self.client.clone();
                let endpoint = endpoint.clone();
                let cached = self.cached_tools(&endpoint.id).await;
                tasks.spawn(async move {
                    let result = match cached {
                        Some(tools) => Ok(tools),
                        None => gateway_client.list_tools(&endpoint).await,
                    };
                    (endpoint.id.clone(), result)
                });
            }
            statuses.push(status);
        }

        let mut fetched: HashMap<String, Result<Vec<Value>, McpError>> = HashMap::new();
        while let Some(joined) = tasks.join_next().await {
            if let Ok((id, result)) = joined {
                fetched.insert(id, result);
            }
        }

        let mut tools = Vec::new();
        for status in &mut statuses {
            let Some(result) = fetched.remove(&status.endpoint_id) else {
                continue;
            };
            let Some(endpoint) = endpoints.iter().find(|e| e.id == status.endpoint_id) else {
                continue;
            };
            match result {
                Ok(raw_tools) => {
                    self.store_cache(&endpoint.id, &raw_tools).await;
                    let allowed = allowed_tool_names(endpoint);
                    for raw in raw_tools {
                        let Some(original) = raw.get("name").and_then(Value::as_str) else {
                            continue;
                        };
                        if let Some(allowed) = &allowed
                            && !allowed.contains(original)
                        {
                            continue;
                        }
                        tools.push(McpToolInfo {
                            name: format!("{}{}{}", status.prefix, TOOL_NAME_SEPARATOR, original),
                            endpoint_id: endpoint.id.clone(),
                            endpoint_name: endpoint.name.clone(),
                            original_name: original.to_string(),
                            description: raw
                                .get("description")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            input_schema: raw
                                .get("inputSchema")
                                .cloned()
                                .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
                        });
                        status.tool_count += 1;
                    }
                }
                Err(error) => status.error = Some(error.message),
            }
        }

        McpToolCatalog {
            tools,
            endpoints: statuses,
        }
    }

    /// 调用方可见的工具（MCP `tools/list` 的内容）
    pub async fn tools_for(&self, caller: &McpCaller) -> Vec<Value> {
        self.catalog(false)
            .await
            .tools
            .into_iter()
            .filter(|tool| caller.can_access(&tool.endpoint_id))
            .map(|tool| {
                let description = match &tool.description {
                    Some(text) if !text.is_empty() => format!("[{}] {}", tool.endpoint_name, text),
                    _ => format!("[{}]", tool.endpoint_name),
                };
                json!({
                    "name": tool.name,
                    "description": description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect()
    }

    /// 调用工具：解析 `{前缀}__{工具名}`，转发到对应的下游 MCP Server
    pub async fn call_tool(
        &self,
        caller: &McpCaller,
        exposed_name: &str,
        arguments: Value,
    ) -> Result<Value, McpError> {
        let started = Instant::now();
        let mut endpoint_id: Option<String> = None;
        let result = self
            .call_tool_inner(caller, exposed_name, arguments, &mut endpoint_id)
            .await;
        let error = match &result {
            Ok(value) if value.get("isError").and_then(Value::as_bool) == Some(true) => {
                Some("工具返回 isError".to_string())
            }
            Ok(_) => None,
            Err(e) => Some(e.message.clone()),
        };
        self.record_call(McpCallRecord {
            id: ulid::Ulid::new().to_string(),
            time: Utc::now(),
            caller: caller.name.clone(),
            tool: exposed_name.to_string(),
            endpoint_id,
            success: error.is_none(),
            duration_ms: started.elapsed().as_millis() as u64,
            error,
        });
        result
    }

    async fn call_tool_inner(
        &self,
        caller: &McpCaller,
        exposed_name: &str,
        arguments: Value,
        endpoint_id: &mut Option<String>,
    ) -> Result<Value, McpError> {
        let unknown = || McpError::invalid_params(format!("未知工具: {exposed_name}"));

        let endpoint = {
            let registry = self.endpoints.read().await;
            registry
                .by_protocol(ProtocolType::Mcp)
                .into_iter()
                .filter(|e| e.enabled)
                .map(|e| (endpoint_prefix(e), e))
                .filter(|(prefix, _)| {
                    exposed_name
                        .strip_prefix(prefix.as_str())
                        .is_some_and(|rest| rest.starts_with(TOOL_NAME_SEPARATOR))
                })
                // 前缀可能互为前缀，取最长匹配；前缀冲突时与目录一致，取 ID 最小的端点
                .min_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.1.id.cmp(&b.1.id)))
                .map(|(_, e)| e.clone())
        }
        .ok_or_else(unknown)?;
        let prefix = endpoint_prefix(&endpoint);
        let original = &exposed_name[prefix.len() + TOOL_NAME_SEPARATOR.len()..];

        if original.is_empty() || !caller.can_access(&endpoint.id) {
            return Err(unknown());
        }
        if let Some(allowed) = allowed_tool_names(&endpoint)
            && !allowed.contains(original)
        {
            return Err(unknown());
        }
        *endpoint_id = Some(endpoint.id.clone());

        let arguments = match arguments {
            Value::Null => json!({}),
            Value::Object(_) => arguments,
            _ => return Err(McpError::invalid_params("arguments 必须是 JSON 对象")),
        };
        self.client.call_tool(&endpoint, original, arguments).await
    }

    /// 最近调用记录（新的在前）
    pub fn recent_calls(&self, limit: usize) -> Vec<McpCallRecord> {
        let calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
        calls.iter().rev().take(limit).cloned().collect()
    }

    fn record_call(&self, record: McpCallRecord) {
        tracing::info!(
            caller = %record.caller,
            tool = %record.tool,
            success = record.success,
            duration_ms = record.duration_ms,
            "MCP tool call"
        );
        let mut calls = self.calls.lock().unwrap_or_else(|p| p.into_inner());
        calls.push_back(record);
        while calls.len() > self.config.call_log_capacity.max(1) {
            calls.pop_front();
        }
    }

    async fn cached_tools(&self, endpoint_id: &str) -> Option<Vec<Value>> {
        let ttl = Duration::from_secs(self.config.tool_cache_secs);
        let cache = self.tool_cache.read().await;
        cache
            .get(endpoint_id)
            .filter(|entry| entry.fetched_at.elapsed() < ttl)
            .map(|entry| entry.tools.clone())
    }

    async fn store_cache(&self, endpoint_id: &str, tools: &[Value]) {
        self.tool_cache.write().await.insert(
            endpoint_id.to_string(),
            CachedTools {
                fetched_at: Instant::now(),
                tools: tools.to_vec(),
            },
        );
    }

    /// 处理智能体发来的 JSON-RPC 请求（支持批量）。返回 `None` 表示无需响应（通知）。
    pub async fn handle_rpc(&self, body: Value, caller: &McpCaller) -> Option<Value> {
        match body {
            Value::Array(items) => {
                if items.is_empty() {
                    return Some(error_response(
                        Value::Null,
                        codes::INVALID_REQUEST,
                        "空的批量请求",
                    ));
                }
                let mut responses = Vec::new();
                for item in items {
                    if let Some(response) = self.handle_single(item, caller).await {
                        responses.push(response);
                    }
                }
                (!responses.is_empty()).then(|| Value::Array(responses))
            }
            single => self.handle_single(single, caller).await,
        }
    }

    async fn handle_single(&self, message: Value, caller: &McpCaller) -> Option<Value> {
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // 客户端发来的响应或无效消息：有 id 则报错，否则忽略
            return id.map(|id| error_response(id, codes::INVALID_REQUEST, "缺少 method 字段"));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        // 通知（无 id）不返回内容
        let Some(id) = id else {
            return None;
        };

        let outcome: Result<Value, McpError> = match method {
            "initialize" => Ok(self.initialize_result(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools_for(caller).await })),
            "tools/call" => self.handle_tools_call(&params, caller).await,
            other => Err(McpError::new(
                codes::METHOD_NOT_FOUND,
                format!("不支持的方法: {other}"),
            )),
        };

        Some(match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => {
                let mut response = error_response(id, error.code, &error.message);
                if let Some(data) = error.data {
                    response["error"]["data"] = data;
                }
                response
            }
        })
    }

    fn initialize_result(&self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = requested
            .filter(|v| SUPPORTED_PROTOCOL_VERSIONS.contains(v))
            .unwrap_or(SUPPORTED_PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "nexus-hsb", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Nexus HSB MCP 网关：聚合多个 MCP 服务，工具名格式为 <服务前缀>__<工具名>。",
        })
    }

    async fn handle_tools_call(
        &self,
        params: &Value,
        caller: &McpCaller,
    ) -> Result<Value, McpError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::invalid_params("缺少工具名 name"))?;
        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

        match self.call_tool(caller, name, arguments).await {
            // 下游不可用属于工具执行失败：以 isError 结果返回，便于智能体看到原因并自行处理
            Err(error) if error.is_upstream() => Ok(json!({
                "content": [{ "type": "text", "text": error.message }],
                "isError": true,
            })),
            other => other,
        }
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// 端点的工具前缀：优先使用 `mcp_tool_prefix`，否则取端点 ID；仅保留字母数字、`_`、`-`
pub fn endpoint_prefix(endpoint: &EndpointInfo) -> String {
    let raw = endpoint
        .properties
        .get("mcp_tool_prefix")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(endpoint.id.as_str());
    sanitize_prefix(raw)
}

/// 规范化前缀，保证不含 `__` 且不以 `_` 结尾（使 `前缀__工具名` 可以无歧义地拆分）
pub fn sanitize_prefix(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let ch = if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            ch
        } else {
            '_'
        };
        if ch == '_' && out.ends_with('_') {
            continue;
        }
        out.push(ch);
    }
    let trimmed = out.trim_end_matches('_').trim_start_matches('_');
    if trimmed.is_empty() {
        "mcp".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 端点允许暴露的工具白名单；`None` 表示不限制
fn allowed_tool_names(endpoint: &EndpointInfo) -> Option<HashSet<String>> {
    let raw = endpoint.properties.get("mcp_allowed_tools")?;
    let names: HashSet<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    if names.is_empty() || names.contains("*") {
        None
    } else {
        Some(names)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let len = a.len().max(b.len());
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= (x ^ y) as usize;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_sanitized_and_unambiguous() {
        assert_eq!(sanitize_prefix("his.core"), "his_core");
        assert_eq!(sanitize_prefix("a__b"), "a_b");
        assert_eq!(sanitize_prefix("lis_"), "lis");
        assert_eq!(sanitize_prefix("___"), "mcp");
        assert_eq!(sanitize_prefix("医院"), "mcp");
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    use crate::transport::{
        HealthStatus, Transport, TransportRegistry, TransportRequest, TransportResponse,
        TransportStats, TransportType,
    };
    use async_trait::async_trait;
    use bytes::Bytes;
    use hsb_common::HsbResult;

    /// 模拟下游 MCP Server：按端点地址返回不同工具；`sse` 地址以 SSE 形式响应
    #[derive(Default)]
    struct FakeMcpTransport {
        log: Mutex<Vec<(String, String, HashMap<String, String>)>>,
    }

    #[async_trait]
    impl Transport for FakeMcpTransport {
        fn transport_type(&self) -> TransportType {
            TransportType::Http
        }
        fn name(&self) -> &str {
            "fake-mcp"
        }
        async fn send(&self, request: TransportRequest) -> HsbResult<TransportResponse> {
            if request.target.contains("down") {
                return Err(hsb_common::HsbError::TransportError {
                    message: "connection refused".to_string(),
                });
            }
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let method = body["method"].as_str().unwrap_or_default().to_string();
            self.log.lock().unwrap().push((
                request.target.clone(),
                method.clone(),
                request.headers.clone(),
            ));

            let mut headers = HashMap::new();
            let Some(id) = body.get("id").cloned() else {
                return Ok(response(202, Bytes::new(), headers));
            };
            let result = match method.as_str() {
                "initialize" => {
                    headers.insert("mcp-session-id".to_string(), "sess-1".to_string());
                    json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"fake","version":"1"}})
                }
                "tools/list" if request.target.contains("his") => json!({"tools":[
                    {"name":"get_patient","description":"查询患者","inputSchema":{"type":"object","properties":{"id":{"type":"string"}}}},
                    {"name":"delete_patient","inputSchema":{"type":"object"}}
                ]}),
                "tools/list" => {
                    json!({"tools":[{"name":"get_order","inputSchema":{"type":"object"}}]})
                }
                "tools/call" => {
                    let name = body["params"]["name"].as_str().unwrap();
                    if name == "boom" {
                        return Ok(response(
                            200,
                            Bytes::from(
                                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"bad args"}})
                                    .to_string(),
                            ),
                            headers,
                        ));
                    }
                    json!({"content":[{"type":"text","text":format!("{} -> {}", request.target, name)}]})
                }
                _ => json!({}),
            };
            let envelope = json!({"jsonrpc":"2.0","id":id,"result":result}).to_string();
            if request.target.contains("lis") {
                headers.insert("content-type".to_string(), "text/event-stream".to_string());
                let sse = format!("event: message\ndata: {envelope}\n\n");
                Ok(response(200, Bytes::from(sse), headers))
            } else {
                headers.insert("content-type".to_string(), "application/json".to_string());
                Ok(response(200, Bytes::from(envelope), headers))
            }
        }
        async fn send_with_timeout(
            &self,
            request: TransportRequest,
            _timeout: Duration,
        ) -> HsbResult<TransportResponse> {
            self.send(request).await
        }
        async fn health_check(&self) -> HsbResult<HealthStatus> {
            Ok(HealthStatus::healthy())
        }
        fn stats(&self) -> TransportStats {
            TransportStats::default()
        }
    }

    fn response(status: u16, body: Bytes, headers: HashMap<String, String>) -> TransportResponse {
        TransportResponse {
            status_code: status,
            body,
            headers,
            duration: Duration::from_millis(1),
            metadata: Default::default(),
        }
    }

    async fn gateway(clients: Vec<McpClientConfig>) -> (McpGateway, Arc<FakeMcpTransport>) {
        let fake = Arc::new(FakeMcpTransport::default());
        let mut transports = TransportRegistry::new();
        transports.register("http", fake.clone());

        let mut registry = EndpointRegistry::new();
        let mut his = EndpointInfo::new(
            "HIS_MCP",
            "HIS 服务",
            ProtocolType::Mcp,
            "http://his.internal/mcp",
        );
        his.properties
            .insert("mcp_tool_prefix".to_string(), "his".to_string());
        his.properties
            .insert("mcp_allowed_tools".to_string(), "get_patient".to_string());
        his.headers
            .insert("Authorization".to_string(), "Bearer downstream".to_string());
        registry.register(his);
        registry.register(EndpointInfo::new(
            "LIS.MCP",
            "LIS 服务",
            ProtocolType::Mcp,
            "http://lis.internal/mcp",
        ));
        registry.register(EndpointInfo::new(
            "PLAIN",
            "REST",
            ProtocolType::Http,
            "http://x",
        ));
        let endpoints = Arc::new(RwLock::new(registry));

        let client = Arc::new(McpClient::new(
            Arc::new(RwLock::new(transports)),
            endpoints.clone(),
        ));
        let config = McpGatewayConfig {
            clients,
            ..Default::default()
        };
        (McpGateway::new(client, endpoints, config), fake)
    }

    #[tokio::test]
    async fn aggregates_tools_from_multiple_servers_and_routes_calls() {
        let (gateway, fake) = gateway(vec![]).await;
        let caller = McpCaller::admin();

        let list = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
                &caller,
            )
            .await
            .unwrap();
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        // delete_patient 被白名单过滤；LIS.MCP 的前缀被规范化；非 MCP 端点不出现
        assert_eq!(names, vec!["his__get_patient", "LIS_MCP__get_order"]);

        let call = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"LIS_MCP__get_order","arguments":{"id":"1"}}}),
                &caller,
            )
            .await
            .unwrap();
        // SSE 响应也能解析
        assert_eq!(
            call["result"]["content"][0]["text"],
            "http://lis.internal/mcp -> get_order"
        );
        let call = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"his__get_patient"}}),
                &caller,
            )
            .await
            .unwrap();
        assert_eq!(
            call["result"]["content"][0]["text"],
            "http://his.internal/mcp -> get_patient"
        );

        let log = fake.log.lock().unwrap();
        let count = |target: &str, method: &str| {
            log.iter()
                .filter(|(t, m, _)| t.contains(target) && m == method)
                .count()
        };
        // 每个下游只握手一次并复用会话
        assert_eq!(count("his", "initialize"), 1);
        assert_eq!(count("lis", "initialize"), 1);
        assert_eq!(count("his", "notifications/initialized"), 1);
        let (_, _, headers) = log
            .iter()
            .find(|(t, m, _)| t.contains("his") && m == "tools/call")
            .unwrap();
        assert_eq!(
            headers.get("Mcp-Session-Id").map(String::as_str),
            Some("sess-1")
        );
        assert_eq!(
            headers.get("Authorization").map(String::as_str),
            Some("Bearer downstream")
        );
        drop(log);

        assert_eq!(gateway.recent_calls(10).len(), 2);
    }

    #[tokio::test]
    async fn rejects_unknown_filtered_and_unauthorized_tools() {
        let (gateway, _) = gateway(vec![]).await;
        let admin = McpCaller::admin();
        for name in ["nope__x", "his__delete_patient", "his__", "get_patient"] {
            let err = gateway
                .call_tool(&admin, name, json!({}))
                .await
                .unwrap_err();
            assert_eq!(err.code, codes::INVALID_PARAMS, "{name}");
        }

        let restricted = McpCaller {
            name: "agent".to_string(),
            allowed_endpoints: vec!["HIS_MCP".to_string()],
        };
        assert!(
            gateway
                .call_tool(&restricted, "LIS_MCP__get_order", json!({}))
                .await
                .is_err()
        );
        let tools = gateway.tools_for(&restricted).await;
        assert_eq!(tools.len(), 1);
        assert!(
            gateway
                .call_tool(&restricted, "his__get_patient", json!([]))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn downstream_errors_are_surfaced_appropriately() {
        let (gateway, _) = gateway(vec![]).await;
        // 下游返回 JSON-RPC 错误：原样透传为错误（工具不在白名单内时先被拒绝，这里用 LIS）
        let response = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"LIS_MCP__boom"}}),
                &McpCaller::admin(),
            )
            .await
            .unwrap();
        assert_eq!(response["error"]["code"], codes::INVALID_PARAMS);

        // 下游不可用：以 isError 结果返回，便于智能体看到原因
        gateway.endpoints.write().await.register(EndpointInfo::new(
            "DOWN",
            "离线服务",
            ProtocolType::Mcp,
            "http://down.internal/mcp",
        ));
        let response = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"DOWN__x"}}),
                &McpCaller::admin(),
            )
            .await
            .unwrap();
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("离线服务")
        );

        // 目录中标出不可用端点及原因，不影响其他端点
        let catalog = gateway.catalog(true).await;
        let down = catalog
            .endpoints
            .iter()
            .find(|e| e.endpoint_id == "DOWN")
            .unwrap();
        assert!(down.error.is_some());
        assert_eq!(catalog.tools.len(), 2);
    }

    #[tokio::test]
    async fn protocol_basics_and_notifications() {
        let (gateway, _) = gateway(vec![]).await;
        let caller = McpCaller::admin();
        let init = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
                &caller,
            )
            .await
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
        assert!(init["result"]["capabilities"]["tools"].is_object());

        assert!(
            gateway
                .handle_rpc(
                    json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                    &caller
                )
                .await
                .is_none()
        );
        let ping = gateway
            .handle_rpc(json!({"jsonrpc":"2.0","id":2,"method":"ping"}), &caller)
            .await
            .unwrap();
        assert_eq!(ping["result"], json!({}));
        let missing = gateway
            .handle_rpc(
                json!({"jsonrpc":"2.0","id":3,"method":"resources/list"}),
                &caller,
            )
            .await
            .unwrap();
        assert_eq!(missing["error"]["code"], codes::METHOD_NOT_FOUND);

        let batch = gateway
            .handle_rpc(json!([{"jsonrpc":"2.0","id":4,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/x"}]), &caller)
            .await
            .unwrap();
        assert_eq!(batch.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn authentication_modes() {
        let (open, _) = gateway(vec![]).await;
        assert_eq!(
            open.authenticate(None, None).unwrap_err(),
            McpAuthError::NotConfigured
        );

        let (gateway, _) = gateway(vec![McpClientConfig {
            name: "triage-agent".to_string(),
            token: "s3cret".to_string(),
            allowed_endpoints: vec!["HIS_MCP".to_string()],
        }])
        .await;
        assert_eq!(
            gateway.authenticate(None, None).unwrap_err(),
            McpAuthError::Unauthorized
        );
        assert_eq!(
            gateway
                .authenticate(Some("Bearer wrong"), None)
                .unwrap_err(),
            McpAuthError::Unauthorized
        );
        let caller = gateway.authenticate(Some("Bearer s3cret"), None).unwrap();
        assert_eq!(caller.name, "triage-agent");
        assert_eq!(caller.allowed_endpoints, vec!["HIS_MCP".to_string()]);
        assert!(gateway.authenticate(None, Some("s3cret")).is_ok());
    }
}
