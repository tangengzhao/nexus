//! HSB 管理模块
//!
//! 提供管理 API、审计日志和基准测试功能。

mod handlers;
mod models;
mod routes;
mod state;

pub mod audit;
pub mod bench;

pub use handlers::*;
pub use models::*;
pub use routes::*;
pub use state::*;

use axum::Router;
use hsb_common::HsbResult;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

/// 管理 API 服务器
pub struct AdminServer {
    config: AdminConfig,
    state: Arc<AdminState>,
}

/// 管理 API 配置
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub port: u16,
    /// 启用 CORS
    pub enable_cors: bool,
    /// API 前缀
    pub api_prefix: String,
    /// 启用认证
    pub enable_auth: bool,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            api_prefix: "/api/v1".to_string(),
            enable_auth: true,
        }
    }
}

impl AdminConfig {
    pub fn address(&self) -> String {
        format!("{}:{}", self.listen_address, self.port)
    }
}

impl AdminServer {
    pub fn new(config: AdminConfig, state: AdminState) -> Self {
        Self {
            config,
            state: Arc::new(state),
        }
    }

    /// 构建路由
    pub fn build_router(&self) -> Router {
        let api_routes = create_api_routes(self.state.clone());

        let mut app = Router::new()
            .nest(&self.config.api_prefix, api_routes)
            .layer(TraceLayer::new_for_http());

        if self.config.enable_cors {
            app = app.layer(CorsLayer::permissive());
        }

        app
    }

    /// 启动服务器
    pub async fn start(&self) -> HsbResult<()> {
        let addr: SocketAddr =
            self.config
                .address()
                .parse()
                .map_err(|e| hsb_common::HsbError::ConfigError {
                    message: format!("Invalid address: {}", e),
                })?;

        let app = self.build_router();

        info!("Starting Admin API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            hsb_common::HsbError::InternalError {
                message: format!("Failed to bind: {}", e),
            }
        })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| hsb_common::HsbError::InternalError {
                message: format!("Server error: {}", e),
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use hsb_common::{
        EndpointEncryptionAlgorithm, EndpointRole, HsbResult, MedicalSystemType, MessageStatus,
        ProtocolType,
    };
    use hsb_core::{
        Endpoint, EndpointRuntimeStatus, EndpointVersionRecord, Message, MessageBuilder, Route,
        engine::{EndpointRegistry, InMemoryRouter},
        persistence::{
            EndpointStore, PersistentMessageQuery, PersistentMessageStore, RouteStore,
            StoredWorkflowDefinition, WorkflowInstanceQuery, WorkflowStore,
        },
        reliability::{CircuitBreakerConfig, CircuitBreakerRegistry, InMemoryDlq},
        workflow::{
            InMemoryWorkflowExecutor, StepType, Workflow, WorkflowContext, WorkflowExecutor,
            WorkflowInstance, WorkflowStep, WorkflowStepHandler,
        },
    };
    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    use crate::audit::{AuditConfig, InMemoryAuditStorage};

    #[derive(Default)]
    struct MockEndpointStore {
        endpoints: RwLock<HashMap<String, Endpoint>>,
        statuses: RwLock<HashMap<String, EndpointRuntimeStatus>>,
        versions: RwLock<HashMap<String, Vec<EndpointVersionRecord>>>,
    }

    #[derive(Default)]
    struct MockRouteStore {
        routes: RwLock<HashMap<String, Route>>,
    }

    #[derive(Default)]
    struct MockMessageStore {
        messages: RwLock<HashMap<String, Message>>,
    }

    #[derive(Default)]
    struct MockMessageReplayService {
        replayed: RwLock<Vec<Message>>,
    }

    #[derive(Default)]
    struct MockWorkflowStore {
        workflows: RwLock<HashMap<String, StoredWorkflowDefinition>>,
        instances: RwLock<HashMap<String, WorkflowInstance>>,
    }

    struct TestWorkflowHandler;

    #[async_trait::async_trait]
    impl EndpointStore for MockEndpointStore {
        async fn create_endpoint(
            &self,
            endpoint: &Endpoint,
            actor: Option<&str>,
            change_note: Option<&str>,
        ) -> hsb_common::HsbResult<()> {
            self.endpoints
                .write()
                .await
                .insert(endpoint.id.to_string(), endpoint.clone());
            self.versions
                .write()
                .await
                .entry(endpoint.id.to_string())
                .or_default()
                .push(EndpointVersionRecord {
                    endpoint_id: endpoint.id.to_string(),
                    version: endpoint.version,
                    snapshot: endpoint.clone(),
                    changed_at: endpoint.updated_at,
                    changed_by: actor.map(|value| value.to_string()),
                    change_note: change_note.map(|value| value.to_string()),
                });
            Ok(())
        }

        async fn get_endpoint(&self, id: &str) -> hsb_common::HsbResult<Option<Endpoint>> {
            Ok(self.endpoints.read().await.get(id).cloned())
        }

        async fn list_endpoints(&self) -> hsb_common::HsbResult<Vec<Endpoint>> {
            let mut items: Vec<_> = self.endpoints.read().await.values().cloned().collect();
            items.sort_by(|left, right| left.name.cmp(&right.name));
            Ok(items)
        }

        async fn update_endpoint(
            &self,
            endpoint: &Endpoint,
            actor: Option<&str>,
            change_note: Option<&str>,
        ) -> hsb_common::HsbResult<()> {
            self.endpoints
                .write()
                .await
                .insert(endpoint.id.to_string(), endpoint.clone());
            self.versions
                .write()
                .await
                .entry(endpoint.id.to_string())
                .or_default()
                .push(EndpointVersionRecord {
                    endpoint_id: endpoint.id.to_string(),
                    version: endpoint.version,
                    snapshot: endpoint.clone(),
                    changed_at: endpoint.updated_at,
                    changed_by: actor.map(|value| value.to_string()),
                    change_note: change_note.map(|value| value.to_string()),
                });
            Ok(())
        }

        async fn delete_endpoint(&self, id: &str) -> hsb_common::HsbResult<()> {
            self.endpoints.write().await.remove(id);
            self.statuses.write().await.remove(id);
            self.versions.write().await.remove(id);
            Ok(())
        }

        async fn list_endpoint_versions(
            &self,
            id: &str,
        ) -> hsb_common::HsbResult<Vec<EndpointVersionRecord>> {
            let mut items = self
                .versions
                .read()
                .await
                .get(id)
                .cloned()
                .unwrap_or_default();
            items.sort_by(|left, right| right.version.cmp(&left.version));
            Ok(items)
        }

        async fn get_endpoint_status(
            &self,
            id: &str,
        ) -> hsb_common::HsbResult<Option<EndpointRuntimeStatus>> {
            Ok(self.statuses.read().await.get(id).cloned())
        }

        async fn upsert_endpoint_status(
            &self,
            status: &EndpointRuntimeStatus,
        ) -> hsb_common::HsbResult<()> {
            self.statuses
                .write()
                .await
                .insert(status.endpoint_id.clone(), status.clone());
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl RouteStore for MockRouteStore {
        async fn save_route(&self, route: &Route) -> hsb_common::HsbResult<()> {
            self.routes
                .write()
                .await
                .insert(route.id.to_string(), route.clone());
            Ok(())
        }

        async fn get_route(&self, id: &str) -> hsb_common::HsbResult<Option<Route>> {
            Ok(self.routes.read().await.get(id).cloned())
        }

        async fn list_routes(&self) -> hsb_common::HsbResult<Vec<Route>> {
            let mut items: Vec<_> = self.routes.read().await.values().cloned().collect();
            items.sort_by(|left, right| right.priority.cmp(&left.priority));
            Ok(items)
        }

        async fn delete_route(&self, id: &str) -> hsb_common::HsbResult<()> {
            self.routes.write().await.remove(id);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl PersistentMessageStore for MockMessageStore {
        async fn save_message(&self, msg: &Message) -> hsb_common::HsbResult<()> {
            self.messages
                .write()
                .await
                .insert(msg.id.to_string(), msg.clone());
            Ok(())
        }

        async fn list_messages(
            &self,
            query: &PersistentMessageQuery,
        ) -> hsb_common::HsbResult<Vec<Message>> {
            let mut messages: Vec<_> = self.messages.read().await.values().cloned().collect();
            messages.retain(|message| {
                query
                    .source_system
                    .as_ref()
                    .map(|value| message.source_system.as_str() == value)
                    .unwrap_or(true)
                    && query
                        .target_system
                        .as_ref()
                        .map(|value| {
                            message.target_system.as_ref().map(|target| target.as_str())
                                == Some(value.as_str())
                        })
                        .unwrap_or(true)
                    && query
                        .message_type
                        .as_ref()
                        .map(|value| message.message_type.as_deref() == Some(value.as_str()))
                        .unwrap_or(true)
                    && query
                        .status
                        .as_ref()
                        .map(|value| format!("{:?}", message.status).eq_ignore_ascii_case(value))
                        .unwrap_or(true)
            });
            messages.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            let offset = query.offset.unwrap_or(0);
            let limit = query.limit.unwrap_or(messages.len());
            Ok(messages.into_iter().skip(offset).take(limit).collect())
        }

        async fn get_message(&self, id: &str) -> hsb_common::HsbResult<Option<Message>> {
            Ok(self.messages.read().await.get(id).cloned())
        }

        async fn delete_message(&self, id: &str) -> hsb_common::HsbResult<()> {
            self.messages.write().await.remove(id);
            Ok(())
        }

        async fn pending_messages(&self, limit: usize) -> hsb_common::HsbResult<Vec<Message>> {
            let mut messages: Vec<_> = self
                .messages
                .read()
                .await
                .values()
                .filter(|message| !message.status.is_terminal())
                .cloned()
                .collect();
            messages.sort_by(|left, right| left.created_at.cmp(&right.created_at));
            messages.truncate(limit);
            Ok(messages)
        }

        async fn save_batch(&self, messages: &[Message]) -> hsb_common::HsbResult<()> {
            let mut writer = self.messages.write().await;
            for message in messages {
                writer.insert(message.id.to_string(), message.clone());
            }
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl MessageReplayService for MockMessageReplayService {
        async fn replay(&self, message: Message) -> hsb_common::HsbResult<()> {
            self.replayed.write().await.push(message);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl WorkflowStore for MockWorkflowStore {
        async fn save_workflow(&self, workflow: &Workflow) -> hsb_common::HsbResult<()> {
            let now = chrono::Utc::now();
            let mut workflows = self.workflows.write().await;
            let created_at = workflows
                .get(&workflow.id)
                .map(|value| value.created_at)
                .unwrap_or(now);
            workflows.insert(
                workflow.id.clone(),
                StoredWorkflowDefinition {
                    workflow: workflow.clone(),
                    created_at,
                    updated_at: now,
                },
            );
            Ok(())
        }

        async fn get_workflow(
            &self,
            id: &str,
        ) -> hsb_common::HsbResult<Option<StoredWorkflowDefinition>> {
            Ok(self.workflows.read().await.get(id).cloned())
        }

        async fn list_workflows(&self) -> hsb_common::HsbResult<Vec<StoredWorkflowDefinition>> {
            let mut items: Vec<_> = self.workflows.read().await.values().cloned().collect();
            items.sort_by(|left, right| left.workflow.name.cmp(&right.workflow.name));
            Ok(items)
        }

        async fn delete_workflow(&self, id: &str) -> hsb_common::HsbResult<()> {
            self.workflows.write().await.remove(id);
            Ok(())
        }

        async fn save_workflow_instance(
            &self,
            instance: &WorkflowInstance,
        ) -> hsb_common::HsbResult<()> {
            self.instances
                .write()
                .await
                .insert(instance.id.to_string(), instance.clone());
            Ok(())
        }

        async fn get_workflow_instance(
            &self,
            id: &str,
        ) -> hsb_common::HsbResult<Option<WorkflowInstance>> {
            Ok(self.instances.read().await.get(id).cloned())
        }

        async fn list_workflow_instances(
            &self,
            query: &WorkflowInstanceQuery,
        ) -> hsb_common::HsbResult<Vec<WorkflowInstance>> {
            let mut items: Vec<_> = self.instances.read().await.values().cloned().collect();
            if let Some(workflow_id) = &query.workflow_id {
                items.retain(|item| &item.workflow_id == workflow_id);
            }
            if let Some(status) = &query.status {
                items.retain(|item| format!("{:?}", item.status).eq_ignore_ascii_case(status));
            }
            items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            let offset = query.offset.unwrap_or(0);
            let limit = query.limit.unwrap_or(items.len());
            Ok(items.into_iter().skip(offset).take(limit).collect())
        }

        async fn delete_workflow_instance(&self, id: &str) -> hsb_common::HsbResult<()> {
            self.instances.write().await.remove(id);
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl WorkflowStepHandler for TestWorkflowHandler {
        async fn execute(
            &self,
            step: &WorkflowStep,
            _context: &mut WorkflowContext,
        ) -> HsbResult<Option<serde_json::Value>> {
            Ok(Some(match &step.step_type {
                StepType::Send { endpoint_id, .. } => json!({ "endpoint_id": endpoint_id }),
                StepType::Wait { duration_ms } => json!({ "duration_ms": duration_ms }),
                _ => json!({ "step": step.id }),
            }))
        }
    }

    fn build_test_router(
        endpoint_store: Option<Arc<MockEndpointStore>>,
        route_store: Option<Arc<MockRouteStore>>,
        message_store: Option<Arc<MockMessageStore>>,
        message_replay: Option<Arc<MockMessageReplayService>>,
    ) -> Router {
        let workflow_store = Arc::new(MockWorkflowStore::default());
        let workflow_executor =
            Arc::new(InMemoryWorkflowExecutor::new(Arc::new(TestWorkflowHandler)));
        let state = AdminState::new(
            Arc::new(InMemoryRouter::new()),
            route_store.map(|store| store as Arc<dyn RouteStore>),
            Arc::new(RwLock::new(EndpointRegistry::new())),
            endpoint_store.map(|store| store as Arc<dyn EndpointStore>),
            None,
            None,
            None,
            None,
            message_store.map(|store| store as Arc<dyn PersistentMessageStore>),
            Some(workflow_store as Arc<dyn WorkflowStore>),
            Arc::new(InMemoryDlq::new(128)),
            message_replay.map(|service| service as Arc<dyn MessageReplayService>),
            Arc::new(InMemoryAuditStorage::new(AuditConfig::default(), 128)),
            Arc::new(CircuitBreakerRegistry::new(CircuitBreakerConfig::default())),
            Some(workflow_executor as Arc<dyn WorkflowExecutor>),
        );

        AdminServer::new(
            AdminConfig {
                api_prefix: "/api/v1".to_string(),
                ..Default::default()
            },
            state,
        )
        .build_router()
    }

    async fn send_json(
        app: &Router,
        method: &str,
        uri: &str,
        payload: Value,
    ) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed")
    }

    async fn send(app: &Router, method: &str, uri: &str) -> axum::response::Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed")
    }

    async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        serde_json::from_slice(&body).expect("body should be valid json")
    }

    #[tokio::test]
    async fn endpoint_admin_api_full_lifecycle_flow() {
        let store = Arc::new(MockEndpointStore::default());
        let app = build_test_router(Some(store), None, None, None);
        let organization_id = "ORG_HOSPITAL_01";
        let system_id = "SYS_HIS_01";
        let endpoint_id = "HIS_ENDPOINT_01";

        let create_org_response = send_json(
            &app,
            "POST",
            "/api/v1/organizations",
            json!({
                "id": organization_id,
                "name": "示例医院",
                "organization_type": "HOSPITAL",
                "description": "三级医院"
            }),
        )
        .await;
        assert_eq!(create_org_response.status(), StatusCode::CREATED);

        let create_system_response = send_json(
            &app,
            "POST",
            "/api/v1/systems",
            json!({
                "id": system_id,
                "organization_id": organization_id,
                "name": "HIS 核心系统",
                "description": "门诊住院一体化",
                "system_type": "HIS",
                "topic_namespace": "hospital.his",
                "topic_prefix": "order"
            }),
        )
        .await;
        assert_eq!(create_system_response.status(), StatusCode::CREATED);

        let create_response = send_json(
            &app,
            "POST",
            "/api/v1/endpoints",
            json!({
                "id": endpoint_id,
                "system_id": system_id,
                "name": "HIS 主接口",
                "description": "核心门诊接口",
                "system_type": "HIS",
                "protocol": "HTTP",
                "roles": ["PRODUCER", "CONSUMER"],
                "connection": {
                    "host": "his-api.internal",
                    "port": 8443,
                    "path": "/api/v1/orders",
                    "tls_enabled": true,
                    "tls_cert_path": null,
                    "connect_timeout_secs": 10,
                    "read_timeout_secs": 30,
                    "write_timeout_secs": 30,
                    "pool_size": 16,
                    "reconnect_interval_secs": 5,
                    "keepalive_secs": 60
                },
                "auth": {
                    "type": "basic",
                    "username": "his_user",
                    "password": "secret"
                },
                "config": {
                    "max_retries": 5,
                    "retry_interval_ms": 1200,
                    "compression_enabled": true,
                    "max_message_size": 1048576,
                    "concurrency_limit": 200,
                    "rate_limit": 80,
                    "circuit_breaker_threshold": 8,
                    "log_body": false
                },
                "enabled": true,
                "security": {
                    "secret_ref": null,
                    "require_tls": true,
                    "encryption_algorithm": "SM3",
                    "allow_insecure_skip_verify": false,
                    "allowed_ip_ranges": ["10.10.0.0/16"],
                    "mask_credentials_in_logs": true,
                    "credential_expires_at": null,
                    "credential_last_rotated_at": null
                },
                "properties": {
                    "region": "outpatient"
                },
                "created_by": "ops-user",
                "change_note": "initial create"
            }),
        )
        .await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created: EndpointResponse = read_json(create_response).await;
        assert_eq!(created.id, endpoint_id);
        assert_eq!(created.organization_id.as_deref(), Some(organization_id));
        assert_eq!(created.system_id.as_deref(), Some(system_id));
        assert_eq!(
            created.roles,
            vec![EndpointRole::Producer, EndpointRole::Consumer]
        );
        assert_eq!(created.version, 1);
        assert_eq!(created.protocol, hsb_common::ProtocolType::Http);
        assert_eq!(created.system_type, MedicalSystemType::His);
        assert_eq!(
            created.security.encryption_algorithm,
            EndpointEncryptionAlgorithm::Sm3
        );
        assert_eq!(created.status.expect("status should exist").healthy, true);

        let initial_status_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/status", endpoint_id),
        )
        .await;
        assert_eq!(initial_status_response.status(), StatusCode::OK);
        let initial_status: EndpointStatusResponse = read_json(initial_status_response).await;
        assert_eq!(initial_status.healthy, true);

        let initial_health_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/health", endpoint_id),
        )
        .await;
        assert_eq!(initial_health_response.status(), StatusCode::OK);
        let initial_health: EndpointHealthResponse = read_json(initial_health_response).await;
        assert_eq!(initial_health.healthy, true);

        let list_response = send(&app, "GET", "/api/v1/endpoints").await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: ListResponse<EndpointResponse> = read_json(list_response).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, endpoint_id);

        let update_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/endpoints/{}", endpoint_id),
            json!({
                "system_id": system_id,
                "name": "HIS 主接口-切换",
                "description": "切换到新链路",
                "roles": ["CONSUMER"],
                "enabled": false,
                "lifecycle_status": "DISABLED",
                "updated_by": "ops-user-2",
                "change_note": "disable for switch"
            }),
        )
        .await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated: EndpointResponse = read_json(update_response).await;
        assert_eq!(updated.version, 2);
        assert_eq!(updated.enabled, false);
        assert_eq!(updated.roles, vec![EndpointRole::Consumer]);

        let status_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/endpoints/{}/status", endpoint_id),
            json!({
                "healthy": false,
                "latency_ms": 150,
                "last_error": "upstream timeout",
                "circuit_state": "open",
                "consecutive_failures": 3
            }),
        )
        .await;
        assert_eq!(status_response.status(), StatusCode::OK);
        let status: EndpointStatusResponse = read_json(status_response).await;
        assert_eq!(status.healthy, false);
        assert_eq!(status.latency_ms, Some(150));
        assert_eq!(status.consecutive_failures, 3);

        let security_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/endpoints/{}/security", endpoint_id),
            json!({
                "secret_ref": "vault://hsb/endpoints/his-primary",
                "require_tls": true,
                "encryption_algorithm": "SM3",
                "allow_insecure_skip_verify": false,
                "allowed_ip_ranges": ["10.20.0.0/16", "10.30.0.0/16"],
                "mask_credentials_in_logs": true,
                "rotated_by": "sec-user",
                "change_note": "rotate endpoint secret"
            }),
        )
        .await;
        assert_eq!(security_response.status(), StatusCode::OK);
        let secured: EndpointResponse = read_json(security_response).await;
        assert_eq!(secured.version, 3);
        assert_eq!(
            secured.security.secret_ref.as_deref(),
            Some("vault://hsb/endpoints/his-primary")
        );
        assert_eq!(
            secured.security.encryption_algorithm,
            EndpointEncryptionAlgorithm::Sm3
        );
        assert_eq!(secured.security.allowed_ip_ranges.len(), 2);

        let health_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/health", endpoint_id),
        )
        .await;
        assert_eq!(health_response.status(), StatusCode::OK);
        let health: EndpointHealthResponse = read_json(health_response).await;
        assert_eq!(health.healthy, false);
        assert_eq!(health.circuit_state.as_deref(), Some("open"));
        assert_eq!(health.consecutive_failures, 3);

        let versions_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/versions", endpoint_id),
        )
        .await;
        assert_eq!(versions_response.status(), StatusCode::OK);
        let versions: ListResponse<EndpointVersionResponse> = read_json(versions_response).await;
        assert_eq!(versions.total, 3);
        assert_eq!(versions.items[0].version, 3);
        assert_eq!(versions.items[1].version, 2);
        assert_eq!(versions.items[2].version, 1);

        let get_response = send(&app, "GET", &format!("/api/v1/endpoints/{}", endpoint_id)).await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let endpoint: EndpointResponse = read_json(get_response).await;
        let endpoint_status = endpoint.status.expect("status should exist");
        assert_eq!(endpoint.version, 3);
        assert_eq!(endpoint.organization_id.as_deref(), Some(organization_id));
        assert_eq!(endpoint.system_id.as_deref(), Some(system_id));
        assert_eq!(
            endpoint_status.last_error.as_deref(),
            Some("upstream timeout")
        );
        assert_eq!(endpoint.auth.expect("auth should exist").auth_type, "basic");

        let delete_response = send(
            &app,
            "DELETE",
            &format!("/api/v1/endpoints/{}", endpoint_id),
        )
        .await;
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let not_found = send(&app, "GET", &format!("/api/v1/endpoints/{}", endpoint_id)).await;
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn endpoint_status_reads_backfill_missing_records() {
        let store = Arc::new(MockEndpointStore::default());
        let app = build_test_router(Some(store.clone()), None, None, None);
        let organization_id = "ORG_BACKFILL_01";
        let system_id = "SYS_BACKFILL_01";
        let endpoint_id = "ENDPOINT_BACKFILL_01";

        let create_org_response = send_json(
            &app,
            "POST",
            "/api/v1/organizations",
            json!({
                "id": organization_id,
                "name": "回填医院",
                "organization_type": "HOSPITAL"
            }),
        )
        .await;
        assert_eq!(create_org_response.status(), StatusCode::CREATED);

        let create_system_response = send_json(
            &app,
            "POST",
            "/api/v1/systems",
            json!({
                "id": system_id,
                "organization_id": organization_id,
                "name": "回填系统",
                "system_type": "HIS"
            }),
        )
        .await;
        assert_eq!(create_system_response.status(), StatusCode::CREATED);

        let create_endpoint_response = send_json(
            &app,
            "POST",
            "/api/v1/endpoints",
            json!({
                "id": endpoint_id,
                "system_id": system_id,
                "name": "回填端点",
                "system_type": "HIS",
                "protocol": "HTTP",
                "roles": ["CONSUMER"],
                "connection": {
                    "host": "backfill.internal",
                    "port": 8080,
                    "path": "/health",
                    "tls_enabled": false,
                    "connect_timeout_secs": 5,
                    "read_timeout_secs": 10,
                    "write_timeout_secs": 10,
                    "pool_size": 4,
                    "reconnect_interval_secs": 5,
                    "keepalive_secs": 30
                },
                "enabled": true
            }),
        )
        .await;
        assert_eq!(create_endpoint_response.status(), StatusCode::CREATED);

        store.statuses.write().await.remove(endpoint_id);

        let status_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/status", endpoint_id),
        )
        .await;
        assert_eq!(status_response.status(), StatusCode::OK);
        let status: EndpointStatusResponse = read_json(status_response).await;
        assert_eq!(status.healthy, true);

        let health_response = send(
            &app,
            "GET",
            &format!("/api/v1/endpoints/{}/health", endpoint_id),
        )
        .await;
        assert_eq!(health_response.status(), StatusCode::OK);
        let health: EndpointHealthResponse = read_json(health_response).await;
        assert_eq!(health.healthy, true);

        let detail_response =
            send(&app, "GET", &format!("/api/v1/endpoints/{}", endpoint_id)).await;
        assert_eq!(detail_response.status(), StatusCode::OK);
        let endpoint: EndpointResponse = read_json(detail_response).await;
        assert_eq!(
            endpoint
                .status
                .expect("status should be backfilled")
                .healthy,
            true
        );

        assert!(store.statuses.read().await.contains_key(endpoint_id));
    }

    #[tokio::test]
    async fn route_admin_api_full_lifecycle_flow() {
        let store = Arc::new(MockRouteStore::default());
        let app = build_test_router(None, Some(store), None, None);
        let route_id = "route_his_to_lis";

        let create_response = send_json(
            &app,
            "POST",
            "/api/v1/routes",
            json!({
                "id": route_id,
                "name": "HIS 到 LIS",
                "description": "检验申请下发",
                "source_system": "HIS",
                "message_type": "ORM\\^O01",
                "protocol": "HTTP",
                "targets": [
                    { "system_id": "LIS_HTTP", "endpoint": "LIS_HTTP", "transport": "http", "timeout_secs": 30 }
                ],
                "transformer_ids": ["normalize-order"],
                "priority": 10,
                "enabled": true,
                "delivery_mode": "at_least_once",
                "timeout_ms": 15000,
                "async_delivery": false,
                "require_ack": true,
                "audit_enabled": true,
                "dlq_on_failure": true
            }),
        )
        .await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created: RouteResponse = read_json(create_response).await;
        assert_eq!(created.id, route_id);
        assert_eq!(created.protocol, Some(hsb_common::ProtocolType::Http));
        assert_eq!(created.transformer_ids, vec!["normalize-order".to_string()]);

        let get_response = send(&app, "GET", &format!("/api/v1/routes/{}", route_id)).await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let fetched: RouteResponse = read_json(get_response).await;
        assert_eq!(fetched.targets.len(), 1);
        assert_eq!(fetched.targets[0].system_id, "LIS_HTTP");

        let update_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/routes/{}", route_id),
            json!({
                "name": "HIS 到 LIS 主路由",
                "source_system": "HIS-NEW",
                "priority": 20,
                "enabled": false,
                "delivery_mode": "exactly_once",
                "timeout_ms": 45000,
                "targets": [
                    { "system_id": "LIS_HTTP", "endpoint": "LIS_HTTP", "transport": "http", "timeout_secs": 45 },
                    { "system_id": "LIS_HTTP_BACKUP", "endpoint": "LIS_HTTP_BACKUP", "transport": "http", "timeout_secs": 45 }
                ]
            }),
        )
        .await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated: RouteResponse = read_json(update_response).await;
        assert_eq!(updated.name, "HIS 到 LIS 主路由");
        assert_eq!(updated.source_system.as_deref(), Some("HIS-NEW"));
        assert_eq!(updated.priority, 20);
        assert_eq!(updated.enabled, false);
        assert_eq!(updated.delivery_mode, hsb_core::DeliveryMode::ExactlyOnce);
        assert_eq!(updated.targets.len(), 2);

        let disable_response = send(
            &app,
            "POST",
            &format!("/api/v1/routes/{}/disable", route_id),
        )
        .await;
        assert_eq!(disable_response.status(), StatusCode::OK);

        let enable_response =
            send(&app, "POST", &format!("/api/v1/routes/{}/enable", route_id)).await;
        assert_eq!(enable_response.status(), StatusCode::OK);

        let list_response = send(&app, "GET", "/api/v1/routes").await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: ListResponse<RouteResponse> = read_json(list_response).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, route_id);

        let delete_response = send(&app, "DELETE", &format!("/api/v1/routes/{}", route_id)).await;
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let not_found = send(&app, "GET", &format!("/api/v1/routes/{}", route_id)).await;
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn workflow_admin_api_full_lifecycle_flow() {
        let app = build_test_router(None, None, None, None);
        let workflow_id = "wf_patient_registration";

        let create_response = send_json(
            &app,
            "POST",
            "/api/v1/workflows",
            json!({
                "id": workflow_id,
                "name": "门诊患者登记",
                "description": "HIS 到 LIS 的标准登记编排",
                "version": 1,
                "enabled": true,
                "timeout_ms": 600000,
                "options": {
                    "persist_state": true,
                    "pausable": true,
                    "max_concurrent_instances": 32,
                    "instance_timeout_secs": 1800
                },
                "compensation": {
                    "mode": "sequential",
                    "timeout_ms": 60000,
                    "continue_on_failure": false
                },
                "steps": [
                    {
                        "id": "step_1",
                        "name": "发送到 HIS",
                        "step_type": {
                            "type": "send",
                            "endpoint_id": "HIS_ENDPOINT",
                            "transformer_ids": ["normalize-patient"]
                        },
                        "config": {
                            "async_execution": false,
                            "skippable": false,
                            "input_mapping": {},
                            "output_mapping": {},
                            "properties": {}
                        },
                        "retry": {
                            "max_attempts": 3,
                            "initial_delay_ms": 1000,
                            "max_delay_ms": 30000,
                            "multiplier": 2.0,
                            "retryable_errors": []
                        },
                        "timeout_ms": 30000,
                        "condition": null,
                        "compensation_step": null,
                        "next_steps": [{ "step_id": "step_2", "condition": null }]
                    },
                    {
                        "id": "step_2",
                        "name": "写入审计日志",
                        "step_type": {
                            "type": "log",
                            "level": "info",
                            "message": "patient registered"
                        },
                        "config": {
                            "async_execution": false,
                            "skippable": false,
                            "input_mapping": {},
                            "output_mapping": {},
                            "properties": {}
                        },
                        "retry": null,
                        "timeout_ms": null,
                        "condition": null,
                        "compensation_step": null,
                        "next_steps": []
                    }
                ]
            }),
        )
        .await;
        assert_eq!(create_response.status(), StatusCode::CREATED);
        let created: WorkflowResponse = read_json(create_response).await;
        assert_eq!(created.id, workflow_id);
        assert_eq!(created.steps.len(), 2);
        assert_eq!(created.timeout_ms, 600000);

        let list_response = send(&app, "GET", "/api/v1/workflows").await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: ListResponse<WorkflowResponse> = read_json(list_response).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, workflow_id);

        let get_response = send(&app, "GET", &format!("/api/v1/workflows/{}", workflow_id)).await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let fetched: WorkflowResponse = read_json(get_response).await;
        assert_eq!(fetched.name, "门诊患者登记");
        assert!(fetched.compensation.is_some());

        let update_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/workflows/{}", workflow_id),
            json!({
                "name": "门诊患者登记主流程",
                "enabled": false,
                "clear_compensation": true,
                "steps": [
                    {
                        "id": "step_1",
                        "name": "等待人工确认",
                        "step_type": {
                            "type": "wait",
                            "duration_ms": 120
                        },
                        "config": {
                            "async_execution": false,
                            "skippable": false,
                            "input_mapping": {},
                            "output_mapping": {},
                            "properties": {}
                        },
                        "retry": null,
                        "timeout_ms": null,
                        "condition": null,
                        "compensation_step": null,
                        "next_steps": []
                    }
                ]
            }),
        )
        .await;
        assert_eq!(update_response.status(), StatusCode::OK);
        let updated: WorkflowResponse = read_json(update_response).await;
        assert_eq!(updated.name, "门诊患者登记主流程");
        assert_eq!(updated.version, 2);
        assert_eq!(updated.steps.len(), 1);
        assert_eq!(updated.enabled, false);
        assert!(updated.compensation.is_none());

        let enable_response = send_json(
            &app,
            "PUT",
            &format!("/api/v1/workflows/{}", workflow_id),
            json!({ "enabled": true }),
        )
        .await;
        assert_eq!(enable_response.status(), StatusCode::OK);

        let start_response = send_json(
            &app,
            "POST",
            &format!("/api/v1/workflows/{}/start", workflow_id),
            json!({
                "source_system": "HIS",
                "target_system": "LIS",
                "protocol": "HTTP",
                "message_type": "ADT_A01",
                "payload": { "patient_id": "P10001" }
            }),
        )
        .await;
        assert_eq!(start_response.status(), StatusCode::CREATED);
        let started: WorkflowInstanceResponse = read_json(start_response).await;
        assert_eq!(started.workflow_id, workflow_id);

        let pause_response = send(
            &app,
            "POST",
            &format!("/api/v1/workflow-instances/{}/pause", started.id),
        )
        .await;
        assert_eq!(pause_response.status(), StatusCode::OK);
        let paused: WorkflowInstanceResponse = read_json(pause_response).await;
        assert_eq!(paused.status, "PAUSED");

        let list_instances = send(&app, "GET", "/api/v1/workflow-instances").await;
        assert_eq!(list_instances.status(), StatusCode::OK);
        let instances: ListResponse<WorkflowInstanceResponse> = read_json(list_instances).await;
        assert_eq!(instances.total, 1);
        assert_eq!(instances.items[0].workflow_id, workflow_id);

        let resume_response = send(
            &app,
            "POST",
            &format!("/api/v1/workflow-instances/{}/resume", started.id),
        )
        .await;
        assert_eq!(resume_response.status(), StatusCode::OK);

        tokio::time::sleep(std::time::Duration::from_millis(180)).await;

        let get_instance = send(
            &app,
            "GET",
            &format!("/api/v1/workflow-instances/{}", started.id),
        )
        .await;
        assert_eq!(get_instance.status(), StatusCode::OK);
        let fetched_instance: WorkflowInstanceResponse = read_json(get_instance).await;
        assert_eq!(fetched_instance.status, "COMPLETED");

        let delete_response = send(
            &app,
            "DELETE",
            &format!("/api/v1/workflows/{}", workflow_id),
        )
        .await;
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let not_found = send(&app, "GET", &format!("/api/v1/workflows/{}", workflow_id)).await;
        assert_eq!(not_found.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn message_admin_api_lists_gets_and_reprocesses_messages() {
        let message_store = Arc::new(MockMessageStore::default());
        let replay = Arc::new(MockMessageReplayService::default());
        let app = build_test_router(
            None,
            None,
            Some(message_store.clone()),
            Some(replay.clone()),
        );

        let mut message = MessageBuilder::new()
            .source_system("HIS")
            .target_system("LIS")
            .protocol(ProtocolType::Http)
            .message_type("ORM^O01")
            .raw_payload(br#"{"order_id":"ORD-1"}"#.to_vec())
            .build()
            .expect("message should build");
        message.update_status(MessageStatus::Failed);
        let message_id = message.id.to_string();
        message_store
            .save_message(&message)
            .await
            .expect("message should save");

        let list_response = send(&app, "GET", "/api/v1/messages").await;
        assert_eq!(list_response.status(), StatusCode::OK);
        let listed: ListResponse<MessageResponse> = read_json(list_response).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, message_id);
        assert_eq!(listed.items[0].source_system, "HIS");

        let get_response = send(&app, "GET", &format!("/api/v1/messages/{}", message_id)).await;
        assert_eq!(get_response.status(), StatusCode::OK);
        let fetched: MessageResponse = read_json(get_response).await;
        assert_eq!(fetched.id, message_id);
        assert_eq!(fetched.target_system.as_deref(), Some("LIS"));

        let reprocess_response = send(
            &app,
            "POST",
            &format!("/api/v1/messages/{}/reprocess", message_id),
        )
        .await;
        assert_eq!(reprocess_response.status(), StatusCode::ACCEPTED);

        let replayed = replay.replayed.read().await;
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].id.to_string(), message_id);
        assert_eq!(replayed[0].meta.retry_count, 1);
    }
}
