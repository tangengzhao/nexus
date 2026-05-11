mod profiles;

pub use profiles::{EndpointProfile, EndpointRole, profile_by_id};

use anyhow::{Context, Result, anyhow};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes as BytePayload;
use chrono::Utc;
use futures_util::StreamExt;
use rand::Rng;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::convert::Infallible;
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tonic::Code;
use tonic::transport::Server;
use tower::{ServiceExt, make::Shared, service_fn};
use tracing::{error, info, warn};
use ulid::Ulid;

pub mod proto {
    tonic::include_proto!("mockendpoint");
}

use proto::mock_endpoint_service_client::MockEndpointServiceClient;
use proto::mock_endpoint_service_server::{MockEndpointService, MockEndpointServiceServer};
use proto::{CapabilitiesReply, CapabilitiesRequest, MockMessageReply, MockMessageRequest};

#[derive(Debug)]
struct RuntimeStats {
    total: AtomicU64,
    success: AtomicU64,
    failure: AtomicU64,
    http_requests: AtomicU64,
    grpc_requests: AtomicU64,
    nats_messages: AtomicU64,
}

impl RuntimeStats {
    fn new() -> Self {
        Self {
            total: AtomicU64::new(0),
            success: AtomicU64::new(0),
            failure: AtomicU64::new(0),
            http_requests: AtomicU64::new(0),
            grpc_requests: AtomicU64::new(0),
            nats_messages: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
struct AppState {
    profile: EndpointProfile,
    stats: Arc<RuntimeStats>,
}

impl AppState {
    fn new(profile: EndpointProfile) -> Self {
        Self {
            profile,
            stats: Arc::new(RuntimeStats::new()),
        }
    }

    async fn process_message(
        &self,
        transport: &'static str,
        incoming: IncomingMessage,
    ) -> ProcessingDecision {
        let latency_ms = rand::thread_rng().gen_range(8..90);
        tokio::time::sleep(Duration::from_millis(latency_ms)).await;

        self.stats.total.fetch_add(1, Ordering::Relaxed);
        match transport {
            "HTTP" => {
                self.stats.http_requests.fetch_add(1, Ordering::Relaxed);
            }
            "gRPC" => {
                self.stats.grpc_requests.fetch_add(1, Ordering::Relaxed);
            }
            "NATS" => {
                self.stats.nats_messages.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        let accepted = rand::thread_rng().gen_bool(resolve_accept_rate());
        let protocol = incoming
            .protocol
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let message_id = incoming
            .message_id
            .unwrap_or_else(|| Ulid::new().to_string());
        let trace_id = incoming.trace_id.unwrap_or_else(|| Ulid::new().to_string());

        if accepted {
            self.stats.success.fetch_add(1, Ordering::Relaxed);
            ProcessingDecision::Success(SuccessPayload {
                endpoint_id: self.profile.endpoint_id,
                service_name: self.profile.service_name.to_string(),
                role: self.profile.role.as_str().to_string(),
                transport: transport.to_string(),
                protocol,
                message_id,
                trace_id,
                processing_time_ms: latency_ms,
                timestamp: Utc::now().to_rfc3339(),
                received_bytes: incoming.raw_payload.len() as u64,
                message_type: incoming.message_type,
                source: incoming.source,
                target: incoming.target,
                scenario: incoming.scenario,
                body_preview: incoming.body_preview,
                scenario_tags: self
                    .profile
                    .scenario_tags
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            })
        } else {
            self.stats.failure.fetch_add(1, Ordering::Relaxed);
            let failure = random_failure();
            ProcessingDecision::Failure(FailurePayload {
                endpoint_id: self.profile.endpoint_id,
                service_name: self.profile.service_name.to_string(),
                role: self.profile.role.as_str().to_string(),
                transport: transport.to_string(),
                protocol,
                message_id,
                trace_id,
                processing_time_ms: latency_ms,
                timestamp: Utc::now().to_rfc3339(),
                failure,
                received_bytes: incoming.raw_payload.len() as u64,
                body_preview: incoming.body_preview,
                scenario_tags: self
                    .profile
                    .scenario_tags
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            })
        }
    }

    fn capabilities(&self) -> CapabilitiesReply {
        CapabilitiesReply {
            endpoint_id: self.profile.endpoint_id.to_string(),
            service_name: self.profile.service_name.to_string(),
            role: self.profile.role.as_str().to_string(),
            port: self.profile.port as u32,
            supported_protocols: self
                .profile
                .supported_protocols
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
            scenario_tags: self
                .profile
                .scenario_tags
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
            transports: vec![
                "HTTP".to_string(),
                "gRPC".to_string(),
                "NATS(optional)".to_string(),
            ],
            description: self.profile.description.to_string(),
        }
    }

    fn stats_json(&self) -> Value {
        json!({
            "endpoint_id": self.profile.endpoint_id,
            "service_name": self.profile.service_name,
            "role": self.profile.role.as_str(),
            "total": self.stats.total.load(Ordering::Relaxed),
            "success": self.stats.success.load(Ordering::Relaxed),
            "failure": self.stats.failure.load(Ordering::Relaxed),
            "http_requests": self.stats.http_requests.load(Ordering::Relaxed),
            "grpc_requests": self.stats.grpc_requests.load(Ordering::Relaxed),
            "nats_messages": self.stats.nats_messages.load(Ordering::Relaxed),
        })
    }
}

#[derive(Debug, Clone)]
struct FailureCatalog {
    code: &'static str,
    category: &'static str,
    message: &'static str,
    http_status: u16,
    grpc_code: Code,
    retriable: bool,
}

fn random_failure() -> FailureCatalog {
    const FAILURES: &[FailureCatalog] = &[
        FailureCatalog {
            code: "DB_TIMEOUT",
            category: "database",
            message: "数据库连接超时，事务未建立。",
            http_status: 504,
            grpc_code: Code::DeadlineExceeded,
            retriable: true,
        },
        FailureCatalog {
            code: "SYSTEM_BUSY",
            category: "system",
            message: "系统繁忙，工作线程池已饱和。",
            http_status: 503,
            grpc_code: Code::Unavailable,
            retriable: true,
        },
        FailureCatalog {
            code: "MESSAGE_PARSE_ERROR",
            category: "payload",
            message: "消息结构解析错误，字段层级不符合预期。",
            http_status: 422,
            grpc_code: Code::InvalidArgument,
            retriable: false,
        },
        FailureCatalog {
            code: "SCHEMA_VALIDATION_FAILED",
            category: "validation",
            message: "模式校验失败，缺少必填字段。",
            http_status: 400,
            grpc_code: Code::InvalidArgument,
            retriable: false,
        },
        FailureCatalog {
            code: "DUPLICATE_MESSAGE",
            category: "idempotency",
            message: "检测到重复消息，幂等锁冲突。",
            http_status: 409,
            grpc_code: Code::AlreadyExists,
            retriable: true,
        },
        FailureCatalog {
            code: "DOWNSTREAM_UNAVAILABLE",
            category: "dependency",
            message: "下游依赖服务不可用。",
            http_status: 503,
            grpc_code: Code::Unavailable,
            retriable: true,
        },
        FailureCatalog {
            code: "QUEUE_PUBLISH_REJECTED",
            category: "mq",
            message: "消息队列暂时拒绝写入，触发背压。",
            http_status: 503,
            grpc_code: Code::ResourceExhausted,
            retriable: true,
        },
        FailureCatalog {
            code: "AUTH_CONTEXT_EXPIRED",
            category: "security",
            message: "鉴权上下文过期，无法继续处理。",
            http_status: 401,
            grpc_code: Code::Unauthenticated,
            retriable: false,
        },
        FailureCatalog {
            code: "ROUTE_RESOLUTION_FAILED",
            category: "routing",
            message: "无法解析目标业务路由。",
            http_status: 404,
            grpc_code: Code::NotFound,
            retriable: false,
        },
        FailureCatalog {
            code: "PROTOCOL_BRIDGE_FAILED",
            category: "protocol",
            message: "协议转换失败，目标格式无法构造。",
            http_status: 502,
            grpc_code: Code::FailedPrecondition,
            retriable: true,
        },
        FailureCatalog {
            code: "RATE_LIMITED",
            category: "traffic",
            message: "触发限流阈值，请稍后重试。",
            http_status: 429,
            grpc_code: Code::ResourceExhausted,
            retriable: true,
        },
        FailureCatalog {
            code: "PARTIAL_REFERENCE_DATA_MISSING",
            category: "master-data",
            message: "主数据映射缺失，无法补全业务编码。",
            http_status: 424,
            grpc_code: Code::FailedPrecondition,
            retriable: false,
        },
    ];

    let index = rand::thread_rng().gen_range(0..FAILURES.len());
    FAILURES[index].clone()
}

#[derive(Debug)]
struct IncomingMessage {
    message_id: Option<String>,
    trace_id: Option<String>,
    protocol: Option<String>,
    message_type: Option<String>,
    source: Option<String>,
    target: Option<String>,
    scenario: Option<String>,
    raw_payload: Vec<u8>,
    body_preview: String,
}

impl IncomingMessage {
    fn from_http(headers: &HeaderMap, body: &[u8]) -> Self {
        let body_preview = String::from_utf8_lossy(body)
            .chars()
            .take(256)
            .collect::<String>();
        let payload_json = serde_json::from_slice::<Value>(body).ok();
        Self {
            message_id: header_value(headers, "x-message-id").or_else(|| {
                payload_json
                    .as_ref()
                    .and_then(|v| read_string(v, "message_id"))
            }),
            trace_id: header_value(headers, "x-trace-id"),
            protocol: header_value(headers, "x-hsb-protocol")
                .or_else(|| header_value(headers, "content-type"))
                .or_else(|| {
                    payload_json
                        .as_ref()
                        .and_then(|v| read_string(v, "protocol"))
                }),
            message_type: header_value(headers, "x-hsb-message-type").or_else(|| {
                payload_json
                    .as_ref()
                    .and_then(|v| read_string(v, "message_type"))
            }),
            source: header_value(headers, "x-hsb-source")
                .or_else(|| payload_json.as_ref().and_then(|v| read_string(v, "source"))),
            target: header_value(headers, "x-hsb-target")
                .or_else(|| payload_json.as_ref().and_then(|v| read_string(v, "target"))),
            scenario: header_value(headers, "x-hsb-scenario").or_else(|| {
                payload_json
                    .as_ref()
                    .and_then(|v| read_string(v, "scenario"))
            }),
            raw_payload: body.to_vec(),
            body_preview,
        }
    }

    fn from_grpc(request: MockMessageRequest, trace_id: Option<String>) -> Self {
        let body_preview = if !request.payload_json.is_empty() {
            request.payload_json.chars().take(256).collect::<String>()
        } else {
            String::from_utf8_lossy(&request.raw_payload)
                .chars()
                .take(256)
                .collect::<String>()
        };

        Self {
            message_id: empty_to_none(request.message_id),
            trace_id,
            protocol: empty_to_none(request.protocol),
            message_type: empty_to_none(request.message_type),
            source: empty_to_none(request.source),
            target: empty_to_none(request.target),
            scenario: empty_to_none(request.scenario),
            raw_payload: if request.raw_payload.is_empty() {
                request.payload_json.into_bytes()
            } else {
                request.raw_payload
            },
            body_preview,
        }
    }

    fn from_nats(subject: &str, payload: &[u8]) -> Self {
        let body_preview = String::from_utf8_lossy(payload)
            .chars()
            .take(256)
            .collect::<String>();
        let payload_json = serde_json::from_slice::<Value>(payload).ok();
        Self {
            message_id: payload_json
                .as_ref()
                .and_then(|v| read_string(v, "message_id")),
            trace_id: payload_json
                .as_ref()
                .and_then(|v| read_string(v, "trace_id")),
            protocol: payload_json
                .as_ref()
                .and_then(|v| read_string(v, "protocol"))
                .or_else(|| Some("NATS".to_string())),
            message_type: payload_json
                .as_ref()
                .and_then(|v| read_string(v, "message_type")),
            source: Some(subject.to_string()),
            target: payload_json.as_ref().and_then(|v| read_string(v, "target")),
            scenario: payload_json
                .as_ref()
                .and_then(|v| read_string(v, "scenario")),
            raw_payload: payload.to_vec(),
            body_preview,
        }
    }
}

#[derive(Debug)]
struct SuccessPayload {
    endpoint_id: u8,
    service_name: String,
    role: String,
    transport: String,
    protocol: String,
    message_id: String,
    trace_id: String,
    processing_time_ms: u64,
    timestamp: String,
    received_bytes: u64,
    message_type: Option<String>,
    source: Option<String>,
    target: Option<String>,
    scenario: Option<String>,
    body_preview: String,
    scenario_tags: Vec<String>,
}

#[derive(Debug)]
struct FailurePayload {
    endpoint_id: u8,
    service_name: String,
    role: String,
    transport: String,
    protocol: String,
    message_id: String,
    trace_id: String,
    processing_time_ms: u64,
    timestamp: String,
    failure: FailureCatalog,
    received_bytes: u64,
    body_preview: String,
    scenario_tags: Vec<String>,
}

enum ProcessingDecision {
    Success(SuccessPayload),
    Failure(FailurePayload),
}

impl ProcessingDecision {
    fn into_http_response(self) -> Response {
        match self {
            Self::Success(success) => (
                StatusCode::OK,
                Json(json!({
                    "accepted": true,
                    "status": "SUCCESS",
                    "endpoint": success.endpoint_id,
                    "service_name": success.service_name,
                    "role": success.role,
                    "transport": success.transport,
                    "protocol": success.protocol,
                    "message_id": success.message_id,
                    "trace_id": success.trace_id,
                    "processing_time_ms": success.processing_time_ms,
                    "timestamp": success.timestamp,
                    "received_bytes": success.received_bytes,
                    "message_type": success.message_type,
                    "source": success.source,
                    "target": success.target,
                    "scenario": success.scenario,
                    "scenario_tags": success.scenario_tags,
                    "body_preview": success.body_preview,
                })),
            )
                .into_response(),
            Self::Failure(failure) => {
                let status = StatusCode::from_u16(failure.failure.http_status)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                (
                    status,
                    Json(json!({
                        "accepted": false,
                        "status": "FAILURE",
                        "endpoint": failure.endpoint_id,
                        "service_name": failure.service_name,
                        "role": failure.role,
                        "transport": failure.transport,
                        "protocol": failure.protocol,
                        "message_id": failure.message_id,
                        "trace_id": failure.trace_id,
                        "processing_time_ms": failure.processing_time_ms,
                        "timestamp": failure.timestamp,
                        "received_bytes": failure.received_bytes,
                        "scenario_tags": failure.scenario_tags,
                        "body_preview": failure.body_preview,
                        "error": {
                            "code": failure.failure.code,
                            "category": failure.failure.category,
                            "message": failure.failure.message,
                            "retriable": failure.failure.retriable,
                        }
                    })),
                )
                    .into_response()
            }
        }
    }

    fn into_grpc_result(self) -> Result<tonic::Response<MockMessageReply>, tonic::Status> {
        match self {
            Self::Success(success) => Ok(tonic::Response::new(MockMessageReply {
                endpoint_id: success.endpoint_id.to_string(),
                service_name: success.service_name,
                role: success.role,
                transport: success.transport,
                status: "SUCCESS".to_string(),
                accepted: true,
                message_id: success.message_id,
                trace_id: success.trace_id,
                protocol: success.protocol,
                http_status: 200,
                processing_time_ms: success.processing_time_ms,
                timestamp: success.timestamp,
                scenario_tags: success.scenario_tags,
                failure: None,
            })),
            Self::Failure(failure) => Err(tonic::Status::new(
                failure.failure.grpc_code,
                format!("{}: {}", failure.failure.code, failure.failure.message),
            )),
        }
    }

    fn to_ack_json(&self) -> Value {
        match self {
            Self::Success(success) => json!({
                "accepted": true,
                "status": "SUCCESS",
                "endpoint": success.endpoint_id,
                "service_name": success.service_name,
                "transport": success.transport,
                "protocol": success.protocol,
                "message_id": success.message_id,
                "trace_id": success.trace_id,
                "processing_time_ms": success.processing_time_ms,
                "timestamp": success.timestamp,
            }),
            Self::Failure(failure) => json!({
                "accepted": false,
                "status": "FAILURE",
                "endpoint": failure.endpoint_id,
                "service_name": failure.service_name,
                "transport": failure.transport,
                "protocol": failure.protocol,
                "message_id": failure.message_id,
                "trace_id": failure.trace_id,
                "processing_time_ms": failure.processing_time_ms,
                "timestamp": failure.timestamp,
                "error": {
                    "code": failure.failure.code,
                    "category": failure.failure.category,
                    "message": failure.failure.message,
                    "retriable": failure.failure.retriable,
                }
            }),
        }
    }
}

#[derive(Clone)]
struct GrpcMockService {
    state: AppState,
}

#[tonic::async_trait]
impl MockEndpointService for GrpcMockService {
    async fn handle_message(
        &self,
        request: tonic::Request<MockMessageRequest>,
    ) -> Result<tonic::Response<MockMessageReply>, tonic::Status> {
        let trace_id = request
            .metadata()
            .get("x-trace-id")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let incoming = IncomingMessage::from_grpc(request.into_inner(), trace_id);
        self.state
            .process_message("gRPC", incoming)
            .await
            .into_grpc_result()
    }

    async fn get_capabilities(
        &self,
        _request: tonic::Request<CapabilitiesRequest>,
    ) -> Result<tonic::Response<CapabilitiesReply>, tonic::Status> {
        Ok(tonic::Response::new(self.state.capabilities()))
    }
}

pub async fn run_default_endpoint(endpoint_id: u8) -> Result<()> {
    dotenvy::dotenv().ok();
    let profile = profile_by_id(endpoint_id)
        .ok_or_else(|| anyhow!("unknown endpoint id: {}", endpoint_id))?;
    run_endpoint(profile).await
}

pub async fn run_endpoint(profile: EndpointProfile) -> Result<()> {
    init_tracing();
    let state = AppState::new(resolve_profile(profile));
    let bind_addr = format!("0.0.0.0:{}", state.profile.port);
    let listener = TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", bind_addr))?;

    let mut background_tasks = Vec::new();
    if let Some(task) = spawn_optional_nats_consumer(state.clone()) {
        background_tasks.push(task);
    }
    if let Some(task) = spawn_optional_producer(state.clone()) {
        background_tasks.push(task);
    }

    let grpc_service = Server::builder()
        .add_service(MockEndpointServiceServer::new(GrpcMockService {
            state: state.clone(),
        }))
        .into_service();
    let http_router = build_http_router(state.clone());
    let service = service_fn(move |request: Request<Body>| {
        let grpc_service = grpc_service.clone();
        let http_router = http_router.clone();
        async move {
            if is_grpc_request(&request) {
                match grpc_service.oneshot(request.map(tonic::body::boxed)).await {
                    Ok(response) => Ok::<_, Infallible>(response.map(Body::new)),
                    Err(error) => {
                        error!(error = %error, "gRPC multiplexer request failed");
                        Ok((StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response())
                    }
                }
            } else {
                match http_router.oneshot(request).await {
                    Ok(response) => Ok(response),
                    Err(never) => match never {},
                }
            }
        }
    });

    info!(
        endpoint_id = state.profile.endpoint_id,
        service_name = state.profile.service_name,
        role = state.profile.role.as_str(),
        port = state.profile.port,
        "mock endpoint service started"
    );

    axum::serve(listener, Shared::new(service)).await?;

    for task in background_tasks {
        task.abort();
    }

    Ok(())
}

fn build_http_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root_info))
        .route("/health", get(health))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/messages", post(handle_http_message))
        .route("/api/v1/messages/raw", post(handle_http_message))
        .route("/api/v1/events", post(handle_http_message))
        .with_state(state)
}

async fn root_info(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "service_name": state.profile.service_name,
        "endpoint_id": state.profile.endpoint_id,
        "port": state.profile.port,
        "role": state.profile.role.as_str(),
        "description": state.profile.description,
        "supported_protocols": state.profile.supported_protocols,
        "scenario_tags": state.profile.scenario_tags,
        "routes": ["/health", "/api/v1/capabilities", "/api/v1/stats", "/api/v1/messages"],
    }))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "UP",
        "endpoint_id": state.profile.endpoint_id,
        "service_name": state.profile.service_name,
        "port": state.profile.port,
        "role": state.profile.role.as_str(),
        "stats": state.stats_json(),
    }))
}

async fn capabilities(State(state): State<AppState>) -> Json<Value> {
    let capabilities = state.capabilities();
    Json(json!({
        "endpoint_id": capabilities.endpoint_id,
        "service_name": capabilities.service_name,
        "role": capabilities.role,
        "port": capabilities.port,
        "supported_protocols": capabilities.supported_protocols,
        "scenario_tags": capabilities.scenario_tags,
        "transports": capabilities.transports,
        "description": capabilities.description,
    }))
}

async fn stats(State(state): State<AppState>) -> Json<Value> {
    Json(state.stats_json())
}

async fn handle_http_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let incoming = IncomingMessage::from_http(&headers, &body);
    state
        .process_message("HTTP", incoming)
        .await
        .into_http_response()
}

fn is_grpc_request(request: &Request<Body>) -> bool {
    request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.starts_with("application/grpc"))
        .unwrap_or(false)
}

fn header_value(headers: &HeaderMap, key: &str) -> Option<String> {
    headers
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

fn read_string(payload: &Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

fn resolve_profile(mut profile: EndpointProfile) -> EndpointProfile {
    let port_key = format!("HSB_MOCK_ENDPOINT_{}_PORT", profile.endpoint_id);
    if let Ok(port) = env::var(&port_key) {
        if let Ok(parsed) = port.parse::<u16>() {
            profile.port = parsed;
        }
    }
    profile
}

fn resolve_accept_rate() -> f64 {
    env::var("HSB_MOCK_ACCEPT_RATE")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.70)
}

fn spawn_optional_producer(state: AppState) -> Option<JoinHandle<()>> {
    let endpoint_id = state.profile.endpoint_id;
    let target_key = format!("HSB_MOCK_ENDPOINT_{}_PRODUCER_TARGET", endpoint_id);
    let target = env::var(&target_key).ok()?;
    let protocol = env::var(format!(
        "HSB_MOCK_ENDPOINT_{}_PRODUCER_PROTOCOL",
        endpoint_id
    ))
    .unwrap_or_else(|_| "http".to_string())
    .to_lowercase();
    let interval_ms = env::var(format!(
        "HSB_MOCK_ENDPOINT_{}_PRODUCER_INTERVAL_MS",
        endpoint_id
    ))
    .ok()
    .and_then(|value| value.parse::<u64>().ok())
    .unwrap_or(5000);

    Some(tokio::spawn(async move {
        info!(endpoint_id, target = %target, protocol = %protocol, interval_ms, "producer loop enabled");
        match protocol.as_str() {
            "grpc" => producer_loop_grpc(state, target, interval_ms).await,
            "nats" => producer_loop_nats(state, target, interval_ms).await,
            _ => producer_loop_http(state, target, interval_ms).await,
        }
    }))
}

async fn producer_loop_http(state: AppState, target: String, interval_ms: u64) {
    let client = reqwest::Client::new();
    loop {
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        let message = producer_message(&state, "REST");
        match client.post(&target).json(&message).send().await {
            Ok(response) => {
                info!(endpoint_id = state.profile.endpoint_id, status = %response.status(), target = %target, "produced HTTP test message")
            }
            Err(error) => {
                warn!(endpoint_id = state.profile.endpoint_id, target = %target, error = %error, "failed to produce HTTP test message")
            }
        }
    }
}

async fn producer_loop_grpc(state: AppState, target: String, interval_ms: u64) {
    loop {
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        match MockEndpointServiceClient::connect(target.clone()).await {
            Ok(mut client) => {
                let message = producer_message(&state, "gRPC");
                let request = tonic::Request::new(MockMessageRequest {
                    message_id: message["message_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    protocol: message["protocol"].as_str().unwrap_or_default().to_string(),
                    message_type: message["message_type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    source: message["source"].as_str().unwrap_or_default().to_string(),
                    target: message["target"].as_str().unwrap_or_default().to_string(),
                    scenario: message["scenario"].as_str().unwrap_or_default().to_string(),
                    content_type: "application/json".to_string(),
                    payload_json: message.to_string(),
                    raw_payload: Vec::new(),
                    headers: HashMap::new(),
                });
                if let Err(error) = client.handle_message(request).await {
                    warn!(endpoint_id = state.profile.endpoint_id, target = %target, error = %error, "failed to produce gRPC test message");
                } else {
                    info!(endpoint_id = state.profile.endpoint_id, target = %target, "produced gRPC test message");
                }
            }
            Err(error) => {
                warn!(endpoint_id = state.profile.endpoint_id, target = %target, error = %error, "failed to connect gRPC producer target")
            }
        }
    }
}

async fn producer_loop_nats(state: AppState, subject: String, interval_ms: u64) {
    let url = env::var(format!(
        "HSB_MOCK_ENDPOINT_{}_NATS_URL",
        state.profile.endpoint_id
    ))
    .unwrap_or_else(|_| {
        env::var("HSB_NATS_URLS").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
    });

    loop {
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        match async_nats::connect(&url).await {
            Ok(client) => {
                let message = producer_message(&state, "NATS");
                if let Err(error) = client
                    .publish(subject.clone(), BytePayload::from(message.to_string()))
                    .await
                {
                    warn!(endpoint_id = state.profile.endpoint_id, subject = %subject, error = %error, "failed to publish NATS test message");
                } else {
                    info!(endpoint_id = state.profile.endpoint_id, subject = %subject, "published NATS test message");
                }
            }
            Err(error) => {
                warn!(endpoint_id = state.profile.endpoint_id, url = %url, error = %error, "failed to connect NATS producer target")
            }
        }
    }
}

fn producer_message(state: &AppState, transport: &str) -> Value {
    json!({
        "message_id": Ulid::new().to_string(),
        "trace_id": Ulid::new().to_string(),
        "protocol": transport,
        "message_type": format!("TEST.EVENT.{}", state.profile.endpoint_id),
        "source": state.profile.service_name,
        "target": "hsb-or-target-service",
        "scenario": state.profile.scenario_tags.first().copied().unwrap_or("generic"),
        "payload": {
            "endpoint_id": state.profile.endpoint_id,
            "service_name": state.profile.service_name,
            "role": state.profile.role.as_str(),
            "generated_at": Utc::now().to_rfc3339(),
        }
    })
}

fn spawn_optional_nats_consumer(state: AppState) -> Option<JoinHandle<()>> {
    let subject_key = format!(
        "HSB_MOCK_ENDPOINT_{}_NATS_SUBJECT",
        state.profile.endpoint_id
    );
    let subject = env::var(&subject_key).ok()?;
    let url = env::var(format!(
        "HSB_MOCK_ENDPOINT_{}_NATS_URL",
        state.profile.endpoint_id
    ))
    .unwrap_or_else(|_| {
        env::var("HSB_NATS_URLS").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string())
    });
    let ack_subject = env::var(format!(
        "HSB_MOCK_ENDPOINT_{}_NATS_ACK_SUBJECT",
        state.profile.endpoint_id
    ))
    .ok();

    Some(tokio::spawn(async move {
        match async_nats::connect(&url).await {
            Ok(client) => match client.subscribe(subject.clone()).await {
                Ok(mut subscription) => {
                    info!(endpoint_id = state.profile.endpoint_id, subject = %subject, "NATS consumer enabled");
                    while let Some(message) = subscription.next().await {
                        let incoming = IncomingMessage::from_nats(&subject, &message.payload);
                        let decision = state.process_message("NATS", incoming).await;
                        let ack_json = decision.to_ack_json().to_string();
                        if let Some(reply) = message.reply.clone() {
                            if let Err(error) = client
                                .publish(reply, BytePayload::from(ack_json.clone()))
                                .await
                            {
                                warn!(endpoint_id = state.profile.endpoint_id, error = %error, "failed to reply to NATS message");
                            }
                        } else if let Some(ack_subject) = ack_subject.clone() {
                            if let Err(error) = client
                                .publish(ack_subject.clone(), BytePayload::from(ack_json.clone()))
                                .await
                            {
                                warn!(endpoint_id = state.profile.endpoint_id, ack_subject = %ack_subject, error = %error, "failed to publish NATS ack");
                            }
                        }
                    }
                }
                Err(error) => {
                    error!(endpoint_id = state.profile.endpoint_id, subject = %subject, error = %error, "failed to subscribe NATS subject")
                }
            },
            Err(error) => {
                error!(endpoint_id = state.profile.endpoint_id, url = %url, error = %error, "failed to connect NATS consumer")
            }
        }
    }))
}
