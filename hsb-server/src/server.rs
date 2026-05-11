//! HSB 服务器

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::RwLock,
};
use tracing::{error, info, warn};
use ulid::Ulid;

use hsb_admin::audit::{AuditConfig, InMemoryAuditStorage};
use hsb_admin::{AdminConfig, AdminServer, AdminState, MessageReplayService};
use hsb_common::{
    HsbError, HsbResult, MessageStatus, ProtocolType, TraceId, constants::http as http_headers,
    sso_client::SSOClient,
};
use hsb_core::engine::{
    self, DefaultDispatcher, Dispatcher as _, EndpointRegistry, InMemoryRouter, LoggingProcessor,
    MetricsProcessor, ProcessingPipeline, Router as _, ValidationProcessor,
};
use hsb_core::persistence::{
    EndpointStore, IdempotencyStore, PersistentMessageStore, PgStore, PostgresConfig, RedbStore,
    RouteStore, WorkflowStore,
};
use hsb_core::reliability::{CircuitBreakerConfig, CircuitBreakerRegistry, InMemoryDlq};
use hsb_core::workflow::{
    InMemoryWorkflowExecutor, StepType, WorkflowContext, WorkflowExecutor, WorkflowStep,
    WorkflowStepHandler,
};
use hsb_core::{Message, MessageBuilder, MessageContext};

use crate::config::ServerConfig;

const SESSION_COOKIE_NAME: &str = "hsb_session";
const STATE_COOKIE_NAME: &str = "hsb_sso_state";
const UI_INDEX_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/index.html"));
const UI_APP_JS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/app.js"));

#[derive(Clone)]
struct HttpAppState {
    route_prefix: String,
    sso_enabled: bool,
    sso: Option<Arc<SsoWebState>>,
    message_runtime: Arc<MessageIngressRuntime>,
}

#[derive(Clone)]
struct MessageIngressRuntime {
    adapters: Arc<engine::AdapterRegistry>,
    pipeline: Arc<ProcessingPipeline>,
    router: Arc<InMemoryRouter>,
    dispatcher: Arc<DefaultDispatcher>,
    message_store: Option<Arc<dyn PersistentMessageStore>>,
}

struct SsoWebState {
    client: Arc<SSOClient>,
    scope: String,
    sessions: Arc<RwLock<HashMap<String, WebSession>>>,
}

struct WebSession {
    access_token: String,
    user_name: String,
}

#[derive(Debug, Serialize)]
struct AuthMeResponse {
    authenticated: bool,
    user_name: Option<String>,
    sso_enabled: bool,
}

struct ServerWorkflowHandler;

#[async_trait::async_trait]
impl WorkflowStepHandler for ServerWorkflowHandler {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &mut WorkflowContext,
    ) -> HsbResult<Option<serde_json::Value>> {
        let output = match &step.step_type {
            StepType::Send {
                endpoint_id,
                transformer_ids,
            } => serde_json::json!({
                "action": "send",
                "endpoint_id": endpoint_id,
                "transformer_ids": transformer_ids,
                "input_message_id": context.input_message.as_ref().map(|msg| msg.id.to_string()),
            }),
            StepType::Receive { timeout_ms } => serde_json::json!({
                "action": "receive",
                "timeout_ms": timeout_ms,
            }),
            StepType::Transform { transformer_ids } => serde_json::json!({
                "action": "transform",
                "transformer_ids": transformer_ids,
                "payload": context.input_message.as_ref().and_then(|msg| msg.payload.clone()),
            }),
            StepType::Parallel {
                branches,
                join_mode,
            } => serde_json::json!({
                "action": "parallel",
                "branch_count": branches.len(),
                "join_mode": join_mode,
            }),
            StepType::Choice {
                branches,
                default_branch,
            } => serde_json::json!({
                "action": "choice",
                "branch_count": branches.len(),
                "has_default": default_branch.is_some(),
            }),
            StepType::Script { language, code } => serde_json::json!({
                "action": "script",
                "language": language,
                "code_size": code.len(),
            }),
            StepType::SubWorkflow { workflow_id } => serde_json::json!({
                "action": "sub_workflow",
                "workflow_id": workflow_id,
            }),
            StepType::Wait { duration_ms } => serde_json::json!({
                "action": "wait",
                "duration_ms": duration_ms,
            }),
            StepType::Log { level, message } => serde_json::json!({
                "action": "log",
                "level": level,
                "message": message,
            }),
        };

        Ok(Some(output))
    }
}

#[derive(Debug, Serialize)]
struct InboundMessageResponse {
    message_id: String,
    trace_id: String,
    protocol: String,
    matched_routes: Vec<String>,
    deliveries: Vec<InboundDeliveryResponse>,
}

#[derive(Debug, Serialize)]
struct InboundDeliveryResponse {
    route_id: String,
    target: String,
    success: bool,
    duration_ms: u64,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: String,
    code: String,
}

#[derive(Debug, Deserialize)]
struct AuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// HSB 服务器
pub struct HsbServer {
    config: ServerConfig,
    router: Arc<InMemoryRouter>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    endpoint_store: Option<Arc<PgStore>>,
    dlq: Arc<InMemoryDlq>,
    audit: Arc<InMemoryAuditStorage>,
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    kafka_transport: Option<Arc<hsb_transport::kafka::KafkaTransport>>,
    message_runtime: Arc<MessageIngressRuntime>,
    workflow_executor: Arc<InMemoryWorkflowExecutor>,
}

impl HsbServer {
    /// 创建新服务器
    pub async fn new(config: ServerConfig) -> anyhow::Result<Self> {
        info!("Initializing HSB Server components...");

        // 初始化路由器
        let router = Arc::new(InMemoryRouter::new());
        info!("Router initialized");

        // 初始化端点注册表
        let endpoints = Arc::new(RwLock::new(EndpointRegistry::new()));
        info!("Endpoint registry initialized");

        let endpoint_store = if config.persistence.postgres_enabled {
            let pg_config = PostgresConfig {
                url: config.database.url.clone(),
                max_connections: config.database.max_connections,
                min_connections: config.database.min_connections,
                connect_timeout_secs: config.database.connect_timeout_secs,
                enabled: true,
            };
            let store = Arc::new(PgStore::connect(&pg_config).await?);
            hydrate_endpoint_registry(&endpoints, store.as_ref()).await?;
            hydrate_route_registry(&router, store.as_ref()).await?;
            Some(store)
        } else {
            None
        };

        // 初始化死信队列
        let dlq = Arc::new(InMemoryDlq::new(10000));
        info!("Dead letter queue initialized");

        // 初始化审计服务
        let audit_config = AuditConfig {
            enabled: config.audit.enabled,
            log_message_content: config.audit.log_message_content,
            mask_sensitive_data: config.audit.mask_sensitive_data,
            retention_days: config.audit.retention_days,
            batch_size: config.audit.batch_size,
            ..Default::default()
        };
        let audit = Arc::new(InMemoryAuditStorage::new(audit_config, 100000));
        info!("Audit service initialized");

        // 初始化熔断器注册表
        let cb_config = CircuitBreakerConfig {
            failure_threshold: config.reliability.circuit_breaker_threshold,
            open_duration_secs: config.reliability.circuit_breaker_recovery_secs,
            ..Default::default()
        };
        let circuit_breakers = Arc::new(CircuitBreakerRegistry::new(cb_config));
        info!("Circuit breaker registry initialized");

        let idempotency_store = if config.persistence.redb_enabled {
            Some(Arc::new(RedbStore::open(&config.persistence.redb_path)?))
        } else {
            None
        };
        let (message_runtime, kafka_transport) = build_message_runtime(
            &config,
            router.clone(),
            endpoints.clone(),
            endpoint_store
                .clone()
                .map(|store| store as Arc<dyn PersistentMessageStore>),
            idempotency_store
                .clone()
                .map(|store| store as Arc<dyn IdempotencyStore>),
        )
        .await;
        let message_runtime = Arc::new(message_runtime);
        let workflow_executor = Arc::new(InMemoryWorkflowExecutor::new(Arc::new(
            ServerWorkflowHandler,
        )));

        Ok(Self {
            config,
            router,
            endpoints,
            endpoint_store,
            dlq,
            audit,
            circuit_breakers,
            kafka_transport,
            message_runtime,
            workflow_executor,
        })
    }

    /// 运行服务器
    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Starting HSB Server...");

        self.recover_pending_messages().await?;

        // 启动各个服务
        let mut handles = Vec::new();

        // 启动 Admin API
        if self.config.http.enabled {
            let admin_handle = self.start_admin_api().await?;
            handles.push(admin_handle);
        }

        // 启动 HTTP 入站服务
        if self.config.http.enabled {
            let http_handle = self.start_http_server().await?;
            handles.push(http_handle);
        }

        // 启动 TCP/MLLP 服务
        if self.config.tcp.enabled {
            let tcp_handle = self.start_tcp_server().await?;
            handles.push(tcp_handle);
        }

        // 启动 gRPC 服务
        if self.config.grpc.enabled {
            let grpc_handle = self.start_grpc_server().await?;
            handles.push(grpc_handle);
        }

        info!(
            "HSB Server started successfully. Listening on:\n\
             - HTTP:  {}:{}\n\
             - Admin: {}:{}\n\
             - TCP:   {}:{}\n\
             - gRPC:  {}:{}",
            self.config.http.listen_address,
            self.config.http.port,
            self.config.http.listen_address,
            self.config.http.admin_port,
            self.config.tcp.listen_address,
            self.config.tcp.port,
            self.config.grpc.listen_address,
            self.config.grpc.port,
        );

        // 等待关闭信号
        self.wait_for_shutdown().await?;

        info!("HSB Server shutting down...");

        // 取消所有任务
        for handle in handles {
            handle.abort();
        }

        info!("HSB Server stopped");
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn publish_to_kafka(&self, topic: &str, payload: Bytes) -> anyhow::Result<()> {
        let kafka = self
            .kafka_transport
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Kafka transport is not initialized"))?;

        kafka
            .publish(topic, payload, None, None)
            .await
            .map_err(|e| anyhow::anyhow!("Kafka publish failed: {}", e))
    }

    async fn start_admin_api(&self) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let admin_config = AdminConfig {
            listen_address: self.config.http.listen_address.clone(),
            port: self.config.http.admin_port,
            enable_cors: true,
            api_prefix: join_route_prefix(&self.config.http.route_prefix, "/api/v1"),
            enable_auth: self.config.sso.enabled,
        };

        let admin_state = self.build_admin_state();

        let admin_server = AdminServer::new(admin_config, admin_state);

        let handle = tokio::spawn(async move {
            if let Err(e) = admin_server.start().await {
                error!("Admin API error: {}", e);
            }
        });

        info!("Admin API started on port {}", self.config.http.admin_port);
        Ok(handle)
    }

    async fn start_http_server(&self) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let port = self.config.http.port;
        let listen_addr = self.config.http.listen_address.clone();

        let app_state = self.build_http_app_state()?;
        let route_prefix = app_state.route_prefix.clone();
        let admin_routes = hsb_admin::create_api_routes(Arc::new(self.build_admin_state()));
        let app = Router::new()
            .route("/", get(ui_root))
            .route("/portal", get(home_page))
            .route("/ui", get(ui_index))
            .route("/ui/", get(ui_index))
            .route("/ui/app.js", get(ui_app_js))
            .route("/health", get(health))
            .route("/api/messages/inbound", post(inbound_message))
            .route("/auth/login", get(auth_login))
            .route("/auth/callback", get(auth_callback))
            .route("/auth/logout", get(auth_logout))
            .route("/auth/me", get(auth_me))
            .nest_service("/api/v1", admin_routes)
            .with_state(app_state);
        let app = if route_prefix.is_empty() {
            app
        } else {
            Router::new().nest(&route_prefix, app)
        };

        let bind_addr = format!("{}:{}", listen_addr, port);
        let listener = TcpListener::bind(&bind_addr).await?;

        let handle = tokio::spawn(async move {
            info!(
                "HTTP server listening on {} with route prefix '{}'",
                bind_addr, route_prefix
            );
            if let Err(e) = axum::serve(listener, app).await {
                error!("HTTP server error: {}", e);
            }
        });

        Ok(handle)
    }

    fn build_http_app_state(&self) -> anyhow::Result<HttpAppState> {
        if !self.config.sso.enabled {
            return Ok(HttpAppState {
                route_prefix: self.config.http.route_prefix.clone(),
                sso_enabled: false,
                sso: None,
                message_runtime: self.message_runtime.clone(),
            });
        }

        let client = SSOClient::new(
            &self.config.sso.web_base_url,
            &self.config.sso.client_id,
            &self.config.sso.client_secret,
            &self.config.sso.callback_url,
        )
        .map_err(|e| anyhow::anyhow!("Failed to initialize SSO web client: {}", e))?;

        Ok(HttpAppState {
            route_prefix: self.config.http.route_prefix.clone(),
            sso_enabled: true,
            sso: Some(Arc::new(SsoWebState {
                client: Arc::new(client),
                scope: self.config.sso.scope.clone(),
                sessions: Arc::new(RwLock::new(HashMap::new())),
            })),
            message_runtime: self.message_runtime.clone(),
        })
    }

    fn build_admin_state(&self) -> AdminState {
        AdminState::new(
            self.router.clone(),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn RouteStore>),
            self.endpoints.clone(),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn EndpointStore>),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn hsb_core::persistence::OrganizationStore>),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn hsb_core::persistence::IntegrationSystemStore>),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn PersistentMessageStore>),
            self.endpoint_store
                .clone()
                .map(|store| store as Arc<dyn WorkflowStore>),
            self.dlq.clone(),
            Some(self.message_runtime.clone() as Arc<dyn MessageReplayService>),
            self.audit.clone(),
            self.circuit_breakers.clone(),
            Some(self.workflow_executor.clone() as Arc<dyn WorkflowExecutor>),
        )
    }

    async fn recover_pending_messages(&self) -> anyhow::Result<()> {
        let Some(store) = &self.endpoint_store else {
            return Ok(());
        };

        let pending = store.pending_messages(256).await?;
        if pending.is_empty() {
            return Ok(());
        }

        info!(
            "Recovering {} pending messages from persistence",
            pending.len()
        );

        for message in pending {
            if let Err(err) = self.message_runtime.replay_message(message).await {
                error!("Failed to recover pending message: {}", err);
            }
        }

        Ok(())
    }

    async fn start_tcp_server(&self) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let port = self.config.tcp.port;
        let listen_addr = self.config.tcp.listen_address.clone();
        let bind_addr = format!("{}:{}", listen_addr, port);
        let listener = TcpListener::bind(&bind_addr).await?;

        let handle = tokio::spawn(async move {
            info!("TCP/MLLP server listening on {}", bind_addr);
            loop {
                match listener.accept().await {
                    Ok((mut stream, peer_addr)) => {
                        tokio::spawn(async move {
                            let mut buffer = vec![0_u8; 64 * 1024];
                            loop {
                                match stream.read(&mut buffer).await {
                                    Ok(0) => break,
                                    Ok(bytes_read) => {
                                        info!(
                                            "TCP/MLLP inbound frame received: peer={}, bytes={}",
                                            peer_addr, bytes_read
                                        );
                                        if let Err(e) = stream.write_all(b"ACK\n").await {
                                            warn!("Failed to write TCP/MLLP ACK: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "TCP/MLLP connection error from {}: {}",
                                            peer_addr, e
                                        );
                                        break;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => warn!("TCP/MLLP accept error: {}", e),
                }
            }
        });

        Ok(handle)
    }

    async fn start_grpc_server(&self) -> anyhow::Result<tokio::task::JoinHandle<()>> {
        let port = self.config.grpc.port;
        let listen_addr = self.config.grpc.listen_address.clone();
        let bind_addr = format!("{}:{}", listen_addr, port);
        let listener = TcpListener::bind(&bind_addr).await?;

        let handle = tokio::spawn(async move {
            info!("gRPC ingress listener bound on {}", bind_addr);
            loop {
                match listener.accept().await {
                    Ok((_stream, peer_addr)) => {
                        info!("Accepted gRPC ingress connection from {}", peer_addr);
                    }
                    Err(e) => warn!("gRPC accept error: {}", e),
                }
            }
        });

        Ok(handle)
    }

    async fn wait_for_shutdown(&self) -> anyhow::Result<()> {
        // 等待 Ctrl+C
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}

impl MessageIngressRuntime {
    async fn handle_http_inbound(
        &self,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<InboundMessageResponse, HsbError> {
        let message = self.build_message(headers, body).await?;
        self.process_message(message, false).await
    }

    async fn build_message(&self, headers: &HeaderMap, body: Bytes) -> Result<Message, HsbError> {
        let protocol = resolve_protocol(headers, &body)?;
        let source_system = required_header(headers, http_headers::HEADER_SOURCE_SYSTEM)?;
        let target_system = optional_header(headers, http_headers::HEADER_TARGET_SYSTEM);
        let message_type = optional_header(headers, http_headers::HEADER_MESSAGE_TYPE);
        let correlation_id = optional_header(headers, http_headers::HEADER_CORRELATION_ID);
        let trace_id = optional_header(headers, http_headers::HEADER_TRACE_ID);

        let mut message = match protocol {
            ProtocolType::Http
            | ProtocolType::Custom
            | ProtocolType::OpenAi
            | ProtocolType::Database => build_generic_http_message(
                &source_system,
                target_system.as_deref(),
                message_type.as_deref(),
                correlation_id.as_deref(),
                trace_id.as_deref(),
                protocol,
                headers,
                body,
            )?,
            _ => {
                let adapter = self.adapters.get_required(protocol)?;
                let mut message = adapter
                    .parse(
                        body.clone(),
                        &hsb_core::ParseOptions {
                            preserve_raw: true,
                            ..Default::default()
                        },
                    )
                    .await?;
                message.protocol = protocol;
                message.source_system = source_system.clone().into();
                message.raw_payload = body.to_vec();
                if let Some(target_system) = target_system.as_deref() {
                    message.target_system = Some(target_system.into());
                }
                if let Some(message_type) = message_type.as_ref() {
                    message.message_type = Some(message_type.clone());
                }
                if let Some(correlation_id) = correlation_id.as_ref() {
                    message.correlation_id = Some(correlation_id.clone());
                }
                if let Some(trace_id) = trace_id.as_ref() {
                    message.trace_id = TraceId::from_string(trace_id.clone());
                }
                copy_http_headers(headers, &mut message);
                message.sync_headers();
                message
            }
        };

        message.sync_headers();
        Ok(message)
    }

    async fn replay_message(&self, mut message: Message) -> HsbResult<()> {
        message.update_status(MessageStatus::Retrying);
        let _ = self.process_message(message, true).await?;
        Ok(())
    }

    async fn process_message(
        &self,
        message: Message,
        is_replay: bool,
    ) -> Result<InboundMessageResponse, HsbError> {
        if !is_replay {
            self.persist_message(&message).await?;
        }

        let mut ctx = MessageContext::new(message);
        {
            let mut message = ctx.message_mut().await;
            message.update_status(if is_replay {
                MessageStatus::Retrying
            } else {
                MessageStatus::Processing
            });
        }
        self.persist_context(&ctx).await?;

        if let Err(error) = self.pipeline.execute(&mut ctx).await {
            self.fail_context(&ctx, None).await?;
            return Err(error);
        }

        {
            let mut message = ctx.message_mut().await;
            message.update_status(MessageStatus::Routing);
        }
        self.persist_context(&ctx).await?;

        let message_snapshot = ctx.message().await.clone();
        let routes = self.router.find_routes(&message_snapshot).await?;
        if routes.is_empty() {
            self.fail_context(&ctx, Some("No matched routes".to_string()))
                .await?;
            return Err(HsbError::RouteNotFound {
                message_id: message_snapshot.id,
            });
        }

        {
            let mut message = ctx.message_mut().await;
            message.update_status(MessageStatus::Delivering);
        }
        self.persist_context(&ctx).await?;

        let mut deliveries = Vec::with_capacity(routes.len());
        let matched_routes = routes
            .iter()
            .map(|route| route.id.to_string())
            .collect::<Vec<_>>();
        let mut any_success = false;

        for route in routes {
            match self.dispatcher.dispatch(&ctx, &route).await {
                Ok(result) => {
                    any_success |= result.success;
                    deliveries.push(InboundDeliveryResponse {
                        route_id: route.id.to_string(),
                        target: result.target_system,
                        success: result.success,
                        duration_ms: result.duration_ms,
                        error: result.error,
                    });
                }
                Err(error) => {
                    deliveries.push(InboundDeliveryResponse {
                        route_id: route.id.to_string(),
                        target: route
                            .targets
                            .first()
                            .map(|target| target.endpoint_id.to_string())
                            .unwrap_or_else(|| route.id.to_string()),
                        success: false,
                        duration_ms: 0,
                        error: Some(error.to_string()),
                    });
                }
            }
        }

        {
            let mut message = ctx.message_mut().await;
            if any_success {
                message.update_status(MessageStatus::Completed);
            } else {
                message.meta.dead_letter_reason =
                    Some("All matched routes failed to deliver".to_string());
                message.update_status(MessageStatus::Failed);
            }
        }
        self.persist_context(&ctx).await?;

        if !any_success {
            return Err(HsbError::DeliveryFailed {
                attempts: deliveries.len() as u32,
                reason: "All matched routes failed to deliver".to_string(),
            });
        }

        let message_snapshot = ctx.message().await.clone();
        Ok(InboundMessageResponse {
            message_id: message_snapshot.id.to_string(),
            trace_id: message_snapshot.trace_id.to_string(),
            protocol: message_snapshot.protocol.name().to_string(),
            matched_routes,
            deliveries,
        })
    }

    async fn fail_context(&self, ctx: &MessageContext, reason: Option<String>) -> HsbResult<()> {
        {
            let mut message = ctx.message_mut().await;
            message.meta.dead_letter_reason = reason;
            message.update_status(MessageStatus::Failed);
        }
        self.persist_context(ctx).await
    }

    async fn persist_context(&self, ctx: &MessageContext) -> HsbResult<()> {
        let snapshot = ctx.message().await.clone();
        self.persist_message(&snapshot).await
    }

    async fn persist_message(&self, message: &Message) -> HsbResult<()> {
        if let Some(store) = &self.message_store {
            store.save_message(message).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl MessageReplayService for MessageIngressRuntime {
    async fn replay(&self, message: Message) -> HsbResult<()> {
        self.replay_message(message).await
    }
}

async fn build_message_runtime(
    config: &ServerConfig,
    router: Arc<InMemoryRouter>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    message_store: Option<Arc<dyn PersistentMessageStore>>,
    idempotency_store: Option<Arc<dyn IdempotencyStore>>,
) -> (
    MessageIngressRuntime,
    Option<Arc<hsb_transport::kafka::KafkaTransport>>,
) {
    let adapters = Arc::new(crate::bootstrap::register_adapters(config));
    let (transports, kafka_transport) = crate::bootstrap::register_transports(config).await;
    let transports = Arc::new(RwLock::new(transports));

    let mut pipeline = ProcessingPipeline::new();
    pipeline.add_processor(Arc::new(LoggingProcessor::new(
        config.audit.log_message_content,
    )));
    pipeline.add_processor(Arc::new(MetricsProcessor));
    pipeline.add_processor(Arc::new(ValidationProcessor::new(true)));

    let dispatcher = if let Some(idempotency_store) = idempotency_store {
        Arc::new(DefaultDispatcher::with_idempotency(
            transports,
            endpoints,
            idempotency_store,
        ))
    } else {
        Arc::new(DefaultDispatcher::new(transports, endpoints))
    };

    (
        MessageIngressRuntime {
            adapters,
            pipeline: Arc::new(pipeline),
            router,
            dispatcher,
            message_store,
        },
        kafka_transport,
    )
}

async fn inbound_message(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match state
        .message_runtime
        .handle_http_inbound(&headers, body)
        .await
    {
        Ok(response) => (StatusCode::ACCEPTED, Json(response)).into_response(),
        Err(error) => (
            StatusCode::from_u16(error.http_status_code())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(ApiErrorResponse {
                error: error.to_string(),
                code: error.error_code().to_string(),
            }),
        )
            .into_response(),
    }
}

fn required_header(headers: &HeaderMap, name: &str) -> Result<String, HsbError> {
    optional_header(headers, name).ok_or_else(|| HsbError::MissingConfig {
        key: name.to_string(),
    })
}

fn optional_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn resolve_protocol(headers: &HeaderMap, body: &Bytes) -> Result<ProtocolType, HsbError> {
    if let Some(protocol) = optional_header(headers, "X-HSB-Protocol") {
        return protocol.parse().map_err(|reason| HsbError::InvalidField {
            field: "X-HSB-Protocol".to_string(),
            reason,
        });
    }

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if content_type.contains("application/fhir+") {
        return Ok(ProtocolType::FhirR5);
    }
    if is_hl7v3_body(body) {
        return Ok(ProtocolType::Hl7V3);
    }
    if content_type.contains("soap") || (content_type.contains("text/xml") && is_soap_body(body)) {
        return Ok(ProtocolType::Soap);
    }
    if content_type.contains("application/dicom") {
        return Ok(ProtocolType::Dicom);
    }
    if content_type.contains("application/json") {
        return Ok(ProtocolType::Http);
    }
    if body.starts_with(b"MSH|") {
        return Ok(ProtocolType::Hl7V2);
    }

    Ok(ProtocolType::Http)
}

fn is_hl7v3_body(body: &Bytes) -> bool {
    let content = match std::str::from_utf8(body) {
        Ok(content) => content,
        Err(_) => return false,
    };
    let trimmed = content.trim_start();
    trimmed.starts_with("<") && content.contains("urn:hl7-org:v3") && !is_soap_body(body)
}

fn is_soap_body(body: &Bytes) -> bool {
    let content = match std::str::from_utf8(body) {
        Ok(content) => content.to_ascii_lowercase(),
        Err(_) => return false,
    };
    content.contains(":envelope")
        || content.contains("<envelope")
        || content.contains("soap-envelope")
}

fn build_generic_http_message(
    source_system: &str,
    target_system: Option<&str>,
    message_type: Option<&str>,
    correlation_id: Option<&str>,
    trace_id: Option<&str>,
    protocol: ProtocolType,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Message, HsbError> {
    let mut builder = MessageBuilder::new()
        .source_system(source_system)
        .protocol(protocol)
        .raw_payload(body.to_vec());

    if let Some(target_system) = target_system {
        builder = builder.target_system(target_system);
    }
    if let Some(message_type) = message_type {
        builder = builder.message_type(message_type);
    }
    if let Some(correlation_id) = correlation_id {
        builder = builder.correlation_id(correlation_id);
    }
    if let Some(trace_id) = trace_id {
        builder = builder.trace_id(TraceId::from_string(trace_id.to_string()));
    }

    if let Ok(payload) = serde_json::from_slice::<serde_json::Value>(&body) {
        builder = builder.payload(payload);
    }

    let mut message = builder.build()?;
    copy_http_headers(headers, &mut message);
    Ok(message)
}

fn copy_http_headers(headers: &HeaderMap, message: &mut Message) {
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            message.headers.insert(name.to_string(), value.to_string());
        }
    }
}

async fn hydrate_endpoint_registry(
    registry: &Arc<RwLock<EndpointRegistry>>,
    store: &PgStore,
) -> anyhow::Result<()> {
    let endpoints = store.list_endpoints().await?;
    let mut writer = registry.write().await;
    for endpoint in endpoints {
        let status = store.get_endpoint_status(endpoint.id.as_str()).await?;
        let mut info = hsb_core::engine::EndpointInfo::new(
            endpoint.id.as_str(),
            &endpoint.name,
            endpoint.protocol,
            &endpoint.address(),
        );
        info.enabled = endpoint.enabled;
        info.healthy = status
            .as_ref()
            .map(|value| value.healthy)
            .unwrap_or(endpoint.enabled);
        info.last_heartbeat = status.and_then(|value| value.last_check_at);
        writer.register(info);
    }
    Ok(())
}

async fn hydrate_route_registry(
    router: &Arc<InMemoryRouter>,
    store: &PgStore,
) -> anyhow::Result<()> {
    let routes = store.list_routes().await?;
    for route in routes {
        router.add_route(route).await?;
    }
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn ui_root(State(state): State<HttpAppState>) -> impl IntoResponse {
    Redirect::to(&route_path(&state, "/ui/"))
}

async fn ui_index(State(state): State<HttpAppState>, jar: CookieJar) -> Response {
    if state.sso_enabled && current_session_user(&state, &jar).await.is_none() {
        return Redirect::to(&route_path(&state, "/auth/login")).into_response();
    }

    (
        [(CONTENT_TYPE, "text/html; charset=utf-8")],
        render_ui_index(&state.route_prefix),
    )
        .into_response()
}

async fn ui_app_js() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/javascript; charset=utf-8")],
        UI_APP_JS,
    )
}

async fn home_page(State(state): State<HttpAppState>, jar: CookieJar) -> Response {
    if !state.sso_enabled {
        return Html(render_home_page(Some("Local"), &state.route_prefix)).into_response();
    }

    let Some(sso) = &state.sso else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "SSO 状态初始化失败").into_response();
    };

    let Some(session_cookie) = jar.get(SESSION_COOKIE_NAME) else {
        return Redirect::to(&route_path(&state, "/auth/login")).into_response();
    };

    let user_name = {
        let sessions = sso.sessions.read().await;
        sessions
            .get(session_cookie.value())
            .map(|session| session.user_name.clone())
    };

    match user_name {
        Some(user_name) => {
            Html(render_home_page(Some(&user_name), &state.route_prefix)).into_response()
        }
        None => Redirect::to(&route_path(&state, "/auth/login")).into_response(),
    }
}

async fn auth_login(State(state): State<HttpAppState>, jar: CookieJar) -> Response {
    let Some(sso) = &state.sso else {
        return Redirect::to(&route_path(&state, "/")).into_response();
    };

    let (auth_url, state_value) = sso.client.get_authorization_url(&sso.scope);
    let jar = jar.add(build_cookie(
        &state.route_prefix,
        STATE_COOKIE_NAME,
        state_value,
        true,
    ));

    (jar, Redirect::to(&auth_url)).into_response()
}

async fn auth_callback(
    State(state): State<HttpAppState>,
    jar: CookieJar,
    Query(query): Query<AuthCallbackQuery>,
) -> Response {
    let Some(sso) = &state.sso else {
        return Redirect::to(&route_path(&state, "/")).into_response();
    };

    if let Some(error) = query.error {
        let detail = query.error_description.unwrap_or_default();
        return (
            StatusCode::UNAUTHORIZED,
            format!("SSO 登录失败: {} {}", error, detail),
        )
            .into_response();
    }

    let Some(expected_state_cookie) = jar.get(STATE_COOKIE_NAME) else {
        return (StatusCode::UNAUTHORIZED, "缺少 SSO state cookie").into_response();
    };

    if query.state.as_deref() != Some(expected_state_cookie.value()) {
        return (StatusCode::UNAUTHORIZED, "SSO state 校验失败").into_response();
    }

    let Some(code) = query.code.as_deref() else {
        return (StatusCode::BAD_REQUEST, "缺少 OAuth code 参数").into_response();
    };

    let tokens = match sso.client.exchange_code_for_token(code).await {
        Ok(tokens) => tokens,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("SSO 令牌交换失败: {}", e)).into_response();
        }
    };

    let user = match sso.client.get_user_info(&tokens.access_token).await {
        Ok(user) => user,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("SSO 用户信息获取失败: {}", e),
            )
                .into_response();
        }
    };

    let session_id = Ulid::new().to_string();
    let user_name = resolve_user_name(&user);

    {
        let mut sessions = sso.sessions.write().await;
        sessions.insert(
            session_id.clone(),
            WebSession {
                access_token: tokens.access_token,
                user_name: user_name.clone(),
            },
        );
    }

    info!("User {} logged in through SSO", user_name);

    let jar = jar
        .remove(removal_cookie(&state.route_prefix, STATE_COOKIE_NAME))
        .add(build_cookie(
            &state.route_prefix,
            SESSION_COOKIE_NAME,
            session_id,
            true,
        ));

    (jar, Redirect::to(&route_path(&state, "/ui/"))).into_response()
}

async fn auth_me(State(state): State<HttpAppState>, jar: CookieJar) -> Response {
    let user_name = current_session_user(&state, &jar).await;
    Json(AuthMeResponse {
        authenticated: user_name.is_some() || !state.sso_enabled,
        user_name: user_name.or_else(|| (!state.sso_enabled).then(|| "Local".to_string())),
        sso_enabled: state.sso_enabled,
    })
    .into_response()
}

async fn auth_logout(State(state): State<HttpAppState>, jar: CookieJar) -> Response {
    if let (Some(sso), Some(session_cookie)) = (&state.sso, jar.get(SESSION_COOKIE_NAME)) {
        let access_token = {
            let mut sessions = sso.sessions.write().await;
            sessions
                .remove(session_cookie.value())
                .map(|session| session.access_token)
        };

        if let Some(access_token) = access_token {
            if let Err(e) = sso.client.logout(Some(&access_token)).await {
                warn!("Failed to notify SSO logout: {}", e);
            }
        }
    }

    let jar = jar
        .remove(removal_cookie(&state.route_prefix, SESSION_COOKIE_NAME))
        .remove(removal_cookie(&state.route_prefix, STATE_COOKIE_NAME));

    (jar, Redirect::to(&route_path(&state, "/"))).into_response()
}

fn render_home_page(user_name: Option<&str>, route_prefix: &str) -> String {
    let user_name = user_name.unwrap_or("已登录用户");
    let logout_path = join_route_prefix(route_prefix, "/auth/logout");

    format!(
        "<!DOCTYPE html>\
<html lang=\"zh-CN\">\
<head>\
  <meta charset=\"utf-8\">\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
  <title>HSB 首页</title>\
  <style>\
    body {{ font-family: 'Noto Sans SC', 'PingFang SC', sans-serif; margin: 0; background: linear-gradient(135deg, #f4f7fb, #dce8f6); color: #16324f; }}\
    main {{ max-width: 720px; margin: 8vh auto; padding: 32px; background: rgba(255,255,255,0.92); border-radius: 24px; box-shadow: 0 20px 60px rgba(22, 50, 79, 0.12); }}\
    h1 {{ margin-top: 0; font-size: 2rem; }}\
    p {{ line-height: 1.7; }}\
    a {{ display: inline-block; margin-top: 20px; padding: 12px 18px; border-radius: 999px; background: #16324f; color: #fff; text-decoration: none; }}\
  </style>\
</head>\
<body>\
  <main>\
    <h1>HSB 主页</h1>\
    <p>当前登录用户：{}</p>\
    <p>SSO 登录已完成，主页内容现在可见。</p>\
    <a href=\"{}\">退出登录</a>\
  </main>\
</body>\
</html>",
        user_name, logout_path,
    )
}

fn build_cookie(route_prefix: &str, name: &str, value: String, http_only: bool) -> Cookie<'static> {
    Cookie::build((name.to_string(), value))
        .path(cookie_path(route_prefix))
        .http_only(http_only)
        .same_site(SameSite::Lax)
        .build()
}

fn removal_cookie(route_prefix: &str, name: &str) -> Cookie<'static> {
    Cookie::build((name.to_string(), String::new()))
        .path(cookie_path(route_prefix))
        .http_only(true)
        .same_site(SameSite::Lax)
        .build()
}

fn render_ui_index(route_prefix: &str) -> String {
    UI_INDEX_HTML.replace("__HSB_ROUTE_PREFIX__", route_prefix)
}

fn route_path(state: &HttpAppState, path: &str) -> String {
    join_route_prefix(&state.route_prefix, path)
}

fn join_route_prefix(route_prefix: &str, path: &str) -> String {
    let normalized_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    if route_prefix.is_empty() {
        normalized_path
    } else if normalized_path == "/" {
        format!("{}/", route_prefix.trim_end_matches('/'))
    } else {
        format!("{}{}", route_prefix.trim_end_matches('/'), normalized_path)
    }
}

fn cookie_path(route_prefix: &str) -> String {
    if route_prefix.is_empty() {
        "/".to_string()
    } else {
        route_prefix.to_string()
    }
}

async fn current_session_user(state: &HttpAppState, jar: &CookieJar) -> Option<String> {
    let sso = state.sso.as_ref()?;
    let session_cookie = jar.get(SESSION_COOKIE_NAME)?;
    let sessions = sso.sessions.read().await;
    sessions
        .get(session_cookie.value())
        .map(|session| session.user_name.clone())
}

fn resolve_user_name(user: &hsb_common::sso_client::UserInfo) -> String {
    if !user.name.is_empty() {
        user.name.clone()
    } else if !user.username.is_empty() {
        user.username.clone()
    } else if !user.email.is_empty() {
        user.email.clone()
    } else {
        user.sub.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hsb_core::persistence::IdempotencyStore;
    use hsb_core::persistence::{PersistentMessageQuery, PersistentMessageStore};
    use hsb_core::transport::{
        HealthStatus, Transport, TransportRequest, TransportResponse, TransportStats, TransportType,
    };
    use hsb_core::{
        DeliveryMode, RouteBuilder, RouteOptions, RouteTarget, SourceMatch, TransportRegistry,
    };
    use std::collections::{HashMap, HashSet};
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct MockTransport {
        sent: Arc<Mutex<Vec<(String, Bytes)>>>,
    }

    #[derive(Default)]
    struct MockMessageStore {
        messages: Arc<Mutex<HashMap<String, Message>>>,
    }

    #[derive(Default)]
    struct MockIdempotencyStore {
        keys: Arc<Mutex<HashSet<String>>>,
    }

    #[async_trait]
    impl PersistentMessageStore for MockMessageStore {
        async fn save_message(&self, msg: &Message) -> HsbResult<()> {
            self.messages
                .lock()
                .await
                .insert(msg.id.to_string(), msg.clone());
            Ok(())
        }

        async fn list_messages(&self, _query: &PersistentMessageQuery) -> HsbResult<Vec<Message>> {
            Ok(self.messages.lock().await.values().cloned().collect())
        }

        async fn get_message(&self, id: &str) -> HsbResult<Option<Message>> {
            Ok(self.messages.lock().await.get(id).cloned())
        }

        async fn delete_message(&self, id: &str) -> HsbResult<()> {
            self.messages.lock().await.remove(id);
            Ok(())
        }

        async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>> {
            let mut messages: Vec<_> = self
                .messages
                .lock()
                .await
                .values()
                .filter(|message| !message.status.is_terminal())
                .cloned()
                .collect();
            messages.sort_by(|left, right| left.created_at.cmp(&right.created_at));
            messages.truncate(limit);
            Ok(messages)
        }

        async fn save_batch(&self, messages: &[Message]) -> HsbResult<()> {
            let mut writer = self.messages.lock().await;
            for message in messages {
                writer.insert(message.id.to_string(), message.clone());
            }
            Ok(())
        }
    }

    #[async_trait]
    impl IdempotencyStore for MockIdempotencyStore {
        async fn check_and_mark(&self, idempotency_key: &str, _ttl_secs: u64) -> HsbResult<bool> {
            let mut keys = self.keys.lock().await;
            Ok(keys.insert(idempotency_key.to_string()))
        }

        async fn is_processed(&self, idempotency_key: &str) -> HsbResult<bool> {
            Ok(self.keys.lock().await.contains(idempotency_key))
        }

        async fn clear_mark(&self, idempotency_key: &str) -> HsbResult<()> {
            self.keys.lock().await.remove(idempotency_key);
            Ok(())
        }

        async fn cleanup_expired(&self) -> HsbResult<u64> {
            Ok(0)
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        fn transport_type(&self) -> TransportType {
            TransportType::Http
        }

        fn name(&self) -> &str {
            "mock-http"
        }

        async fn send(&self, request: TransportRequest) -> Result<TransportResponse, HsbError> {
            self.sent
                .lock()
                .await
                .push((request.target.clone(), request.body.clone()));
            Ok(TransportResponse::success(
                Bytes::from_static(b"ok"),
                Duration::from_millis(5),
            ))
        }

        async fn send_with_timeout(
            &self,
            request: TransportRequest,
            _timeout: Duration,
        ) -> Result<TransportResponse, HsbError> {
            self.send(request).await
        }

        async fn health_check(&self) -> Result<HealthStatus, HsbError> {
            Ok(HealthStatus::healthy())
        }

        fn stats(&self) -> TransportStats {
            TransportStats::default()
        }
    }

    #[tokio::test]
    async fn http_inbound_runtime_dispatches_message_to_matched_endpoint() {
        let router = Arc::new(InMemoryRouter::new());
        router
            .add_route(
                RouteBuilder::new()
                    .id("route_his_to_lis")
                    .name("HIS -> LIS HTTP")
                    .source(SourceMatch {
                        system_id: Some("HIS".to_string()),
                        protocol: Some(ProtocolType::Http),
                        message_type_pattern: None,
                    })
                    .target(RouteTarget::primary("LIS_HTTP"))
                    .build()
                    .expect("route should build"),
            )
            .await
            .expect("route should be added");

        let endpoints = Arc::new(RwLock::new(EndpointRegistry::new()));
        endpoints
            .write()
            .await
            .register(hsb_core::engine::EndpointInfo::new(
                "LIS_HTTP",
                "LIS HTTP",
                ProtocolType::Http,
                "http://lis.internal/api/orders",
            ));

        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut transports = TransportRegistry::new();
        transports.register("http", Arc::new(MockTransport { sent: sent.clone() }));

        let mut pipeline = ProcessingPipeline::new();
        pipeline.add_processor(Arc::new(ValidationProcessor::new(true)));
        let message_store = Arc::new(MockMessageStore::default());

        let runtime = MessageIngressRuntime {
            adapters: Arc::new(engine::AdapterRegistry::new()),
            pipeline: Arc::new(pipeline),
            router: router.clone(),
            dispatcher: Arc::new(DefaultDispatcher::new(
                Arc::new(RwLock::new(transports)),
                endpoints,
            )),
            message_store: Some(message_store.clone()),
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            http_headers::HEADER_SOURCE_SYSTEM,
            "HIS".parse().expect("header should parse"),
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/json".parse().expect("header should parse"),
        );

        let response = runtime
            .handle_http_inbound(&headers, Bytes::from_static(br#"{"patient_id":"P001"}"#))
            .await
            .expect("inbound message should be handled");

        assert_eq!(
            response.matched_routes,
            vec!["route_his_to_lis".to_string()]
        );
        assert_eq!(response.deliveries.len(), 1);
        assert!(response.deliveries[0].success);

        let persisted = message_store
            .get_message(&response.message_id)
            .await
            .expect("message lookup should succeed")
            .expect("message should be persisted");
        assert_eq!(persisted.status, MessageStatus::Completed);

        let sent = sent.lock().await;
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, "http://lis.internal/api/orders");
        assert_eq!(sent[0].1, Bytes::from_static(br#"{"patient_id":"P001"}"#));
    }

    #[tokio::test]
    async fn exactly_once_dispatch_suppresses_duplicate_delivery() {
        let router = Arc::new(InMemoryRouter::new());
        router
            .add_route(
                RouteBuilder::new()
                    .id("route_exactly_once")
                    .name("HIS -> LIS ExactlyOnce")
                    .source(SourceMatch {
                        system_id: Some("HIS".to_string()),
                        protocol: Some(ProtocolType::Http),
                        message_type_pattern: None,
                    })
                    .target(RouteTarget::primary("LIS_HTTP"))
                    .options(RouteOptions {
                        delivery_mode: DeliveryMode::ExactlyOnce,
                        ..Default::default()
                    })
                    .build()
                    .expect("route should build"),
            )
            .await
            .expect("route should be added");

        let endpoints = Arc::new(RwLock::new(EndpointRegistry::new()));
        endpoints
            .write()
            .await
            .register(hsb_core::engine::EndpointInfo::new(
                "LIS_HTTP",
                "LIS HTTP",
                ProtocolType::Http,
                "http://lis.internal/api/orders",
            ));

        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut transports = TransportRegistry::new();
        transports.register("http", Arc::new(MockTransport { sent: sent.clone() }));

        let runtime = MessageIngressRuntime {
            adapters: Arc::new(engine::AdapterRegistry::new()),
            pipeline: Arc::new(ProcessingPipeline::new()),
            router,
            dispatcher: Arc::new(DefaultDispatcher::with_idempotency(
                Arc::new(RwLock::new(transports)),
                endpoints,
                Arc::new(MockIdempotencyStore::default()),
            )),
            message_store: None,
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            http_headers::HEADER_SOURCE_SYSTEM,
            "HIS".parse().expect("header should parse"),
        );
        headers.insert(
            http_headers::HEADER_CORRELATION_ID,
            "corr-001".parse().expect("header should parse"),
        );
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/json".parse().expect("header should parse"),
        );

        runtime
            .handle_http_inbound(&headers, Bytes::from_static(br#"{"patient_id":"P001"}"#))
            .await
            .expect("first inbound should succeed");
        runtime
            .handle_http_inbound(&headers, Bytes::from_static(br#"{"patient_id":"P001"}"#))
            .await
            .expect("second inbound should deduplicate");

        let sent = sent.lock().await;
        assert_eq!(sent.len(), 1);
    }

    #[test]
    fn resolve_protocol_prefers_explicit_hl7v3_header() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-HSB-Protocol",
            "HL7V3".parse().expect("header should parse"),
        );

        let protocol = resolve_protocol(&headers, &Bytes::from_static(br#"<ignored/>"#))
            .expect("protocol should resolve");

        assert_eq!(protocol, ProtocolType::Hl7V3);
    }

    #[test]
    fn resolve_protocol_detects_hl7v3_xml_body() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/xml".parse().expect("header should parse"),
        );

        let protocol = resolve_protocol(
            &headers,
            &Bytes::from_static(br#"<PRPA_IN201301UV02 xmlns=\"urn:hl7-org:v3\"/>"#),
        )
        .expect("protocol should resolve");

        assert_eq!(protocol, ProtocolType::Hl7V3);
    }
}
