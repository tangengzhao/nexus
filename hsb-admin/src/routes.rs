//! API 路由定义

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::sync::Arc;

use crate::{AdminState, handlers};

/// 创建 API 路由
pub fn create_api_routes(state: Arc<AdminState>) -> Router {
    Router::new()
        // 健康检查
        .route("/health", get(handlers::health_check))
        .route("/ready", get(handlers::readiness_check))
        // 系统状态
        .route("/status", get(handlers::system_status))
        .route("/metrics", get(handlers::get_metrics))
        // 自定义协议和 Topic 目录
        .route("/custom-protocols", get(handlers::list_custom_protocols))
        .route("/custom-protocols", post(handlers::create_custom_protocol))
        .route("/custom-protocols/{id}", get(handlers::get_custom_protocol))
        .route(
            "/custom-protocols/{id}",
            put(handlers::update_custom_protocol),
        )
        .route(
            "/custom-protocols/{id}",
            delete(handlers::delete_custom_protocol),
        )
        .route("/topics", get(handlers::list_topics))
        .route("/topics", post(handlers::create_topic))
        .route("/topics/{id}", get(handlers::get_topic))
        .route("/topics/{id}", put(handlers::update_topic))
        .route("/topics/{id}", delete(handlers::delete_topic))
        // 机构与系统目录
        .route("/organizations", get(handlers::list_organizations))
        .route("/organizations", post(handlers::create_organization))
        .route("/organizations/{id}", get(handlers::get_organization))
        .route("/organizations/{id}", put(handlers::update_organization))
        .route("/organizations/{id}", delete(handlers::delete_organization))
        .route("/systems", get(handlers::list_systems))
        .route("/systems", post(handlers::create_system))
        .route("/systems/{id}", get(handlers::get_system))
        .route("/systems/{id}", put(handlers::update_system))
        .route("/systems/{id}", delete(handlers::delete_system))
        // 路由管理
        .route("/routes", get(handlers::list_routes))
        .route("/routes", post(handlers::create_route))
        .route("/routes/{id}", get(handlers::get_route))
        .route("/routes/{id}", put(handlers::update_route))
        .route("/routes/{id}", delete(handlers::delete_route))
        .route("/routes/{id}/enable", post(handlers::enable_route))
        .route("/routes/{id}/disable", post(handlers::disable_route))
        // 端点管理
        .route("/endpoints", get(handlers::list_endpoints))
        .route("/endpoints", post(handlers::create_endpoint))
        .route("/endpoints/{id}", get(handlers::get_endpoint))
        .route("/endpoints/{id}", put(handlers::update_endpoint))
        .route("/endpoints/{id}", delete(handlers::delete_endpoint))
        .route(
            "/endpoints/{id}/versions",
            get(handlers::list_endpoint_versions),
        )
        .route("/endpoints/{id}/status", get(handlers::get_endpoint_status))
        .route(
            "/endpoints/{id}/status",
            put(handlers::update_endpoint_status),
        )
        .route(
            "/endpoints/{id}/security",
            put(handlers::update_endpoint_security),
        )
        .route("/endpoints/{id}/health", get(handlers::endpoint_health))
        // 工作流定义
        .route("/workflows", get(handlers::list_workflows))
        .route("/workflows", post(handlers::create_workflow))
        .route("/workflows/{id}", get(handlers::get_workflow))
        .route("/workflows/{id}", put(handlers::update_workflow))
        .route("/workflows/{id}", delete(handlers::delete_workflow))
        .route(
            "/workflows/{id}/start",
            post(handlers::start_workflow_instance),
        )
        .route(
            "/workflow-instances",
            get(handlers::list_workflow_instances),
        )
        .route(
            "/workflow-instances/{id}",
            get(handlers::get_workflow_instance),
        )
        .route(
            "/workflow-instances/{id}/pause",
            post(handlers::pause_workflow_instance),
        )
        .route(
            "/workflow-instances/{id}/resume",
            post(handlers::resume_workflow_instance),
        )
        .route(
            "/workflow-instances/{id}/cancel",
            post(handlers::cancel_workflow_instance),
        )
        .route(
            "/workflow-instances/{id}/compensate",
            post(handlers::compensate_workflow_instance),
        )
        // 消息管理
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route(
            "/messages/{id}/reprocess",
            post(handlers::reprocess_message),
        )
        // 死信队列
        .route("/dlq", get(handlers::list_dlq))
        .route("/dlq/stats", get(handlers::dlq_stats))
        .route("/dlq/{id}", get(handlers::get_dlq_message))
        .route("/dlq/{id}/reprocess", post(handlers::reprocess_dlq_message))
        .route("/dlq/{id}", delete(handlers::delete_dlq_message))
        // 审计日志
        .route("/audit", get(handlers::query_audit))
        .route(
            "/audit/trace/{message_id}",
            get(handlers::get_message_trace),
        )
        // 熔断器
        .route("/circuit-breakers", get(handlers::list_circuit_breakers))
        .route(
            "/circuit-breakers/{name}/reset",
            post(handlers::reset_circuit_breaker),
        )
        // 配置
        .route("/config", get(handlers::get_config))
        .route("/config", put(handlers::update_config))
        .route("/config/reload", post(handlers::reload_config))
        .with_state(state)
}
