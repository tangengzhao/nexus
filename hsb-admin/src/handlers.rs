//! API 处理器

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use std::sync::Arc;

use crate::{AdminState, models::*};

type AppState = State<Arc<AdminState>>;

// ============ 健康检查 ============

/// 健康检查
pub async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now(),
    })
}

/// 就绪检查
pub async fn readiness_check(State(state): AppState) -> impl IntoResponse {
    let ready = state.is_ready().await;
    let status_code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(ReadinessResponse {
            ready,
            checks: state.readiness_checks().await,
        }),
    )
}

// ============ 系统状态 ============

/// 系统状态
pub async fn system_status(State(state): AppState) -> impl IntoResponse {
    let status = state.system_status().await;
    Json(status)
}

/// 获取指标
pub async fn get_metrics(State(state): AppState) -> impl IntoResponse {
    let metrics = state.get_metrics().await;
    Json(metrics)
}

// ============ 自定义协议与 Topic 维护 ============

pub async fn list_custom_protocols(State(state): AppState) -> impl IntoResponse {
    match state.list_custom_protocols().await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListResponse {
                total: items.len(),
                items,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn create_custom_protocol(
    State(state): AppState,
    Json(req): Json<CreateCustomProtocolRequest>,
) -> impl IntoResponse {
    match state.create_custom_protocol(req).await {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_custom_protocol(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_custom_protocol(&id).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_custom_protocol(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateCustomProtocolRequest>,
) -> impl IntoResponse {
    match state.update_custom_protocol(&id, req).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_custom_protocol(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.delete_custom_protocol(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn list_topics(State(state): AppState) -> impl IntoResponse {
    match state.list_topics().await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListResponse {
                total: items.len(),
                items,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn create_topic(
    State(state): AppState,
    Json(req): Json<CreateTopicRequest>,
) -> impl IntoResponse {
    match state.create_topic(req).await {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_topic(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_topic(&id).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_topic(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateTopicRequest>,
) -> impl IntoResponse {
    match state.update_topic(&id, req).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_topic(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.delete_topic(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

// ============ 机构/系统目录 ============

pub async fn list_organizations(State(state): AppState) -> impl IntoResponse {
    match state.list_organizations().await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListResponse {
                total: items.len(),
                items,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn create_organization(
    State(state): AppState,
    Json(req): Json<CreateOrganizationRequest>,
) -> impl IntoResponse {
    match state.create_organization(req).await {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_organization(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_organization(&id).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_organization(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateOrganizationRequest>,
) -> impl IntoResponse {
    match state.update_organization(&id, req).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_organization(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.delete_organization(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn list_systems(State(state): AppState) -> impl IntoResponse {
    match state.list_systems().await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListResponse {
                total: items.len(),
                items,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn create_system(
    State(state): AppState,
    Json(req): Json<CreateIntegrationSystemRequest>,
) -> impl IntoResponse {
    match state.create_system(req).await {
        Ok(item) => (StatusCode::CREATED, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_system(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_system(&id).await {
        Ok(Some(item)) => (StatusCode::OK, Json(item)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_system(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateIntegrationSystemRequest>,
) -> impl IntoResponse {
    match state.update_system(&id, req).await {
        Ok(item) => (StatusCode::OK, Json(item)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_system(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.delete_system(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

// ============ 路由管理 ============

/// 列出所有路由
pub async fn list_routes(State(state): AppState) -> impl IntoResponse {
    match state.list_routes().await {
        Ok(routes) => {
            let total = routes.len();
            (
                StatusCode::OK,
                Json(ListResponse {
                    items: routes,
                    total,
                }),
            )
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ListResponse {
                items: vec![],
                total: 0,
            }),
        ),
    }
}

/// 创建路由
pub async fn create_route(
    State(state): AppState,
    Json(req): Json<CreateRouteRequest>,
) -> impl IntoResponse {
    match state.create_route(req).await {
        Ok(route) => (StatusCode::CREATED, Json(route)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

/// 获取路由
pub async fn get_route(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_route(&id).await {
        Ok(Some(route)) => (StatusCode::OK, Json(route)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

/// 更新路由
pub async fn update_route(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateRouteRequest>,
) -> impl IntoResponse {
    match state.update_route(&id, req).await {
        Ok(route) => (StatusCode::OK, Json(route)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

/// 删除路由
pub async fn delete_route(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.delete_route(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

/// 启用路由
pub async fn enable_route(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.set_route_enabled(&id, true).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

/// 禁用路由
pub async fn disable_route(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.set_route_enabled(&id, false).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

// ============ 端点管理 ============

pub async fn list_endpoints(State(state): AppState) -> impl IntoResponse {
    match state.list_endpoints().await {
        Ok(endpoints) => Json(ListResponse {
            total: endpoints.len(),
            items: endpoints,
        }),
        Err(_) => Json(ListResponse {
            items: vec![],
            total: 0,
        }),
    }
}

pub async fn create_endpoint(
    State(state): AppState,
    Json(req): Json<CreateEndpointRequest>,
) -> impl IntoResponse {
    match state.create_endpoint(req).await {
        Ok(endpoint) => (StatusCode::CREATED, Json(endpoint)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_endpoint(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_endpoint(&id).await {
        Ok(Some(endpoint)) => (StatusCode::OK, Json(endpoint)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_endpoint(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateEndpointRequest>,
) -> impl IntoResponse {
    match state.update_endpoint(&id, req).await {
        Ok(endpoint) => (StatusCode::OK, Json(endpoint)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_endpoint(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.delete_endpoint(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn list_endpoint_versions(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.list_endpoint_versions(&id).await {
        Ok(items) => (
            StatusCode::OK,
            Json(ListResponse {
                total: items.len(),
                items,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn get_endpoint_status(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_endpoint_status(&id).await {
        Ok(Some(status)) => (StatusCode::OK, Json(status)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_endpoint_status(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateEndpointStatusRequest>,
) -> impl IntoResponse {
    match state.update_endpoint_status(&id, req).await {
        Ok(status) => (StatusCode::OK, Json(status)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn update_endpoint_security(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateEndpointSecurityRequest>,
) -> impl IntoResponse {
    match state.update_endpoint_security(&id, req).await {
        Ok(endpoint) => (StatusCode::OK, Json(endpoint)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn endpoint_health(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.check_endpoint_health(&id).await {
        Ok(health) => (StatusCode::OK, Json(health)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

// ============ 工作流管理 ============

pub async fn list_workflows(State(state): AppState) -> impl IntoResponse {
    match state.list_workflows().await {
        Ok(workflows) => Json(ListResponse {
            total: workflows.len(),
            items: workflows,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn create_workflow(
    State(state): AppState,
    Json(req): Json<CreateWorkflowRequest>,
) -> impl IntoResponse {
    match state.create_workflow(req).await {
        Ok(workflow) => (StatusCode::CREATED, Json(workflow)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn get_workflow(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_workflow(&id).await {
        Ok(Some(workflow)) => (StatusCode::OK, Json(workflow)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn update_workflow(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<UpdateWorkflowRequest>,
) -> impl IntoResponse {
    match state.update_workflow(&id, req).await {
        Ok(workflow) => (StatusCode::OK, Json(workflow)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn delete_workflow(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.delete_workflow(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn start_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
    Json(req): Json<StartWorkflowInstanceRequest>,
) -> impl IntoResponse {
    match state.start_workflow_instance(&id, req).await {
        Ok(instance) => (StatusCode::CREATED, Json(instance)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn list_workflow_instances(
    State(state): AppState,
    Query(params): Query<WorkflowInstanceQueryParams>,
) -> impl IntoResponse {
    match state.list_workflow_instances(params).await {
        Ok(instances) => Json(ListResponse {
            total: instances.len(),
            items: instances,
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn get_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.get_workflow_instance(&id).await {
        Ok(Some(instance)) => (StatusCode::OK, Json(instance)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn pause_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.pause_workflow_instance(&id).await {
        Ok(instance) => (StatusCode::OK, Json(instance)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn resume_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.resume_workflow_instance(&id).await {
        Ok(instance) => (StatusCode::OK, Json(instance)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn cancel_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.cancel_workflow_instance(&id).await {
        Ok(instance) => (StatusCode::OK, Json(instance)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn compensate_workflow_instance(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.compensate_workflow_instance(&id).await {
        Ok(instance) => (StatusCode::OK, Json(instance)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

// ============ 消息管理 ============

pub async fn list_messages(
    State(state): AppState,
    Query(params): Query<MessageQueryParams>,
) -> impl IntoResponse {
    match state.list_messages(params).await {
        Ok(messages) => {
            let total = messages.len();
            Json(ListResponse {
                items: messages,
                total,
            })
        }
        Err(_) => Json(ListResponse {
            items: vec![],
            total: 0,
        }),
    }
}

pub async fn get_message(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_message(&id).await {
        Ok(Some(msg)) => (StatusCode::OK, Json(msg)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn reprocess_message(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.reprocess_message(&id).await {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn submit_manual_message(
    State(state): AppState,
    Json(request): Json<ManualMessageRequest>,
) -> impl IntoResponse {
    match state.submit_manual_message(request).await {
        Ok(response) => (StatusCode::ACCEPTED, Json(response)).into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

// ============ 死信队列 ============

pub async fn list_dlq(
    State(state): AppState,
    Query(params): Query<DlqQueryParams>,
) -> impl IntoResponse {
    match state.list_dlq(params).await {
        Ok(items) => {
            let total = items.len();
            Json(ListResponse { items, total })
        }
        Err(_) => Json(ListResponse {
            items: vec![],
            total: 0,
        }),
    }
}

pub async fn dlq_stats(State(state): AppState) -> impl IntoResponse {
    match state.dlq_stats().await {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn get_dlq_message(State(state): AppState, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_dlq_message(&id).await {
        Ok(Some(msg)) => (StatusCode::OK, Json(msg)).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn reprocess_dlq_message(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.reprocess_dlq_message(&id).await {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

pub async fn delete_dlq_message(
    State(state): AppState,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.delete_dlq_message(&id).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

// ============ 审计日志 ============

pub async fn query_audit(
    State(state): AppState,
    Query(params): Query<AuditQueryParams>,
) -> impl IntoResponse {
    match state.query_audit(params).await {
        Ok(events) => Json(ListResponse {
            items: events,
            total: 0,
        }),
        Err(_) => Json(ListResponse {
            items: vec![],
            total: 0,
        }),
    }
}

pub async fn get_message_trace(
    State(state): AppState,
    Path(message_id): Path<String>,
) -> impl IntoResponse {
    match state.get_message_trace(&message_id).await {
        Ok(trace) => (StatusCode::OK, Json(trace)).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(ErrorResponse::from(e))).into_response(),
    }
}

// ============ 熔断器 ============

pub async fn list_circuit_breakers(State(state): AppState) -> impl IntoResponse {
    let breakers = state.list_circuit_breakers().await;
    Json(breakers)
}

pub async fn reset_circuit_breaker(
    State(state): AppState,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.reset_circuit_breaker(&name).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(ErrorResponse::from(e))).into_response(),
    }
}

// ============ 配置 ============

pub async fn get_config(State(state): AppState) -> impl IntoResponse {
    let config = state.get_config().await;
    Json(config)
}

pub async fn update_config(
    State(state): AppState,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    match state.update_config(req).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ErrorResponse::from(e))).into_response(),
    }
}

pub async fn reload_config(State(state): AppState) -> impl IntoResponse {
    match state.reload_config().await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::from(e)),
        )
            .into_response(),
    }
}

// ============ MCP 网关 ============

fn mcp_disabled() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "MCP 网关未启用", "code": "MCP_DISABLED" })),
    )
}

/// 网关概览：路径、认证方式、可访问的智能体
pub async fn mcp_info(State(state): AppState) -> impl IntoResponse {
    let Some(gateway) = state.mcp_gateway() else {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "enabled": false })),
        );
    };
    let config = gateway.config();
    let auth_mode = if !config.clients.is_empty() {
        "tokens"
    } else if config.allow_anonymous {
        "anonymous"
    } else {
        "unconfigured"
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "enabled": true,
            "path": config.path,
            "auth_mode": auth_mode,
            "clients": config.clients.iter().map(|c| serde_json::json!({
                "name": c.name,
                "allowed_endpoints": c.allowed_endpoints,
            })).collect::<Vec<_>>(),
            "tool_cache_secs": config.tool_cache_secs,
            "protocol_versions": hsb_core::mcp::SUPPORTED_PROTOCOL_VERSIONS,
        })),
    )
}

#[derive(Debug, serde::Deserialize)]
pub struct McpToolsQuery {
    pub refresh: Option<bool>,
}

/// 聚合后的工具目录与各 MCP 端点状态
pub async fn mcp_tools(
    State(state): AppState,
    Query(query): Query<McpToolsQuery>,
) -> impl IntoResponse {
    let Some(gateway) = state.mcp_gateway() else {
        return mcp_disabled().into_response();
    };
    let catalog = gateway.catalog(query.refresh.unwrap_or(false)).await;
    (StatusCode::OK, Json(catalog)).into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct McpCallRequest {
    pub tool: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// 调试调用：以管理员身份调用某个聚合工具（与智能体走同一路径）
pub async fn mcp_call(
    State(state): AppState,
    Json(req): Json<McpCallRequest>,
) -> impl IntoResponse {
    let Some(gateway) = state.mcp_gateway() else {
        return mcp_disabled().into_response();
    };
    match gateway
        .call_tool(&hsb_core::mcp::McpCaller::admin(), &req.tool, req.arguments)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => {
            let status = if error.is_upstream() {
                StatusCode::BAD_GATEWAY
            } else {
                StatusCode::BAD_REQUEST
            };
            (
                status,
                Json(serde_json::json!({ "error": error.message, "code": "MCP_CALL_FAILED" })),
            )
                .into_response()
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct McpCallsQuery {
    pub limit: Option<usize>,
}

/// 最近的工具调用记录
pub async fn mcp_calls(
    State(state): AppState,
    Query(query): Query<McpCallsQuery>,
) -> impl IntoResponse {
    let Some(gateway) = state.mcp_gateway() else {
        return mcp_disabled().into_response();
    };
    let items = gateway.recent_calls(query.limit.unwrap_or(50).min(500));
    let total = items.len();
    (StatusCode::OK, Json(ListResponse { items, total })).into_response()
}
