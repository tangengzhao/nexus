//! API 请求/响应模型

use hsb_common::{
    EndpointEncryptionAlgorithm, EndpointRole, MedicalSystemType, OrganizationType, ProtocolType,
};
use hsb_core::workflow::{
    CompensationMode, NextStep, RetryPolicy, StepConfig, StepExecution, StepType, WorkflowContext,
    WorkflowOptions,
};
use hsb_core::{
    AuthConfig, ConnectionConfig, DeliveryMode, EndpointConfig, EndpointLifecycleStatus,
    EndpointSecurity, IntegrationSystem, MatchRule, Organization,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============ 自定义协议与 Topic 维护 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProtocolFieldDefinition {
    pub name: String,
    pub label: Option<String>,
    pub data_type: String,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomProtocolRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub transport_hint: Option<String>,
    pub content_type: Option<String>,
    pub fields: Vec<CustomProtocolFieldDefinition>,
    pub sample_payload: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCustomProtocolRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub transport_hint: Option<String>,
    pub content_type: Option<String>,
    pub fields: Option<Vec<CustomProtocolFieldDefinition>>,
    pub sample_payload: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProtocolResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub transport_hint: Option<String>,
    pub content_type: Option<String>,
    pub fields: Vec<CustomProtocolFieldDefinition>,
    pub sample_payload: Option<serde_json::Value>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTopicRequest {
    pub topic: String,
    pub description: Option<String>,
    pub owner_system_id: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTopicRequest {
    pub topic: Option<String>,
    pub description: Option<String>,
    pub owner_system_id: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicResponse {
    pub id: String,
    pub topic: String,
    pub domain: String,
    pub service: String,
    pub action: String,
    pub version: String,
    pub description: Option<String>,
    pub owner_system_id: Option<String>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ============ 通用响应 ============

/// 健康检查响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 就绪检查响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub ready: bool,
    pub checks: HashMap<String, CheckResult>,
}

/// 检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub status: String,
    pub message: Option<String>,
}

/// 列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub total: usize,
}

/// 错误响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<serde_json::Value>,
}

impl From<hsb_common::HsbError> for ErrorResponse {
    fn from(e: hsb_common::HsbError) -> Self {
        Self {
            error: e.to_string(),
            code: e.error_code().to_string(),
            details: None,
        }
    }
}

// ============ 路由相关 ============

// ============ 机构/系统目录 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub organization_type: OrganizationType,
    pub parent_organization_id: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrganizationRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub organization_type: Option<OrganizationType>,
    pub parent_organization_id: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub organization_type: OrganizationType,
    pub parent_organization_id: Option<String>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIntegrationSystemRequest {
    pub id: Option<String>,
    pub organization_id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_type: MedicalSystemType,
    pub topic_namespace: Option<String>,
    pub topic_prefix: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIntegrationSystemRequest {
    pub organization_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_type: Option<MedicalSystemType>,
    pub topic_namespace: Option<String>,
    pub topic_prefix: Option<String>,
    pub enabled: Option<bool>,
    pub properties: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationSystemResponse {
    pub id: String,
    pub organization_id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_type: MedicalSystemType,
    pub topic_namespace: Option<String>,
    pub topic_prefix: Option<String>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 创建路由请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRouteRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub source_system: Option<String>,
    pub message_type: Option<String>,
    pub protocol: Option<ProtocolType>,
    pub conditions: Option<Vec<MatchRule>>,
    pub targets: Vec<RouteTargetRequest>,
    pub transformer_ids: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub delivery_mode: Option<DeliveryMode>,
    pub timeout_ms: Option<u64>,
    pub async_delivery: Option<bool>,
    pub require_ack: Option<bool>,
    pub audit_enabled: Option<bool>,
    pub dlq_on_failure: Option<bool>,
}

/// 路由目标请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTargetRequest {
    pub system_id: String,
    pub endpoint: String,
    pub transport: Option<String>,
    pub timeout_secs: Option<u64>,
}

/// 更新路由请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRouteRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub source_system: Option<String>,
    pub message_type: Option<String>,
    pub protocol: Option<ProtocolType>,
    pub conditions: Option<Vec<MatchRule>>,
    pub targets: Option<Vec<RouteTargetRequest>>,
    pub transformer_ids: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
    pub delivery_mode: Option<DeliveryMode>,
    pub timeout_ms: Option<u64>,
    pub async_delivery: Option<bool>,
    pub require_ack: Option<bool>,
    pub audit_enabled: Option<bool>,
    pub dlq_on_failure: Option<bool>,
}

/// 路由响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_system: Option<String>,
    pub message_type: Option<String>,
    pub protocol: Option<ProtocolType>,
    pub conditions: Vec<MatchRule>,
    pub targets: Vec<RouteTargetResponse>,
    pub transformer_ids: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
    pub delivery_mode: DeliveryMode,
    pub timeout_ms: u64,
    pub async_delivery: bool,
    pub require_ack: bool,
    pub audit_enabled: bool,
    pub dlq_on_failure: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 路由目标响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTargetResponse {
    pub system_id: String,
    pub endpoint: String,
    pub transport: String,
    pub timeout_secs: u64,
}

// ============ 端点相关 ============

/// 创建端点请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEndpointRequest {
    pub id: Option<String>,
    pub system_id: String,
    pub name: String,
    pub description: Option<String>,
    pub system_type: MedicalSystemType,
    pub protocol: ProtocolType,
    pub roles: Option<Vec<EndpointRole>>,
    pub connection: ConnectionConfig,
    pub auth: Option<AuthConfig>,
    pub config: Option<EndpointConfig>,
    pub enabled: Option<bool>,
    pub lifecycle_status: Option<EndpointLifecycleStatus>,
    pub security: Option<EndpointSecurity>,
    pub properties: Option<HashMap<String, String>>,
    pub created_by: Option<String>,
    pub change_note: Option<String>,
}

/// 更新端点请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEndpointRequest {
    pub system_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub system_type: Option<MedicalSystemType>,
    pub protocol: Option<ProtocolType>,
    pub roles: Option<Vec<EndpointRole>>,
    pub connection: Option<ConnectionConfig>,
    pub auth: Option<AuthConfig>,
    pub config: Option<EndpointConfig>,
    pub enabled: Option<bool>,
    pub lifecycle_status: Option<EndpointLifecycleStatus>,
    pub security: Option<EndpointSecurity>,
    pub properties: Option<HashMap<String, String>>,
    pub updated_by: Option<String>,
    pub change_note: Option<String>,
}

/// 更新端点运行状态请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEndpointStatusRequest {
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub circuit_state: Option<String>,
    pub consecutive_failures: Option<u32>,
    pub last_check_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_delivery_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 更新端点安全配置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEndpointSecurityRequest {
    pub secret_ref: Option<String>,
    pub require_tls: Option<bool>,
    pub encryption_algorithm: Option<EndpointEncryptionAlgorithm>,
    pub allow_insecure_skip_verify: Option<bool>,
    pub allowed_ip_ranges: Option<Vec<String>>,
    pub mask_credentials_in_logs: Option<bool>,
    pub credential_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub rotated_by: Option<String>,
    pub change_note: Option<String>,
}

/// 认证摘要响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointAuthSummaryResponse {
    pub auth_type: String,
    pub principal: Option<String>,
    pub header_name: Option<String>,
    pub token_url: Option<String>,
    pub scope: Option<String>,
    pub external_reference: Option<String>,
    pub secret_configured: bool,
}

/// 端点响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointResponse {
    pub id: String,
    pub organization_id: Option<String>,
    pub system_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub system_type: MedicalSystemType,
    pub protocol: ProtocolType,
    pub roles: Vec<EndpointRole>,
    pub connection: ConnectionConfig,
    pub auth: Option<EndpointAuthSummaryResponse>,
    pub config: EndpointConfig,
    pub enabled: bool,
    pub lifecycle_status: EndpointLifecycleStatus,
    pub version: u32,
    pub security: EndpointSecurity,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Option<String>,
    pub updated_by: Option<String>,
    pub status: Option<EndpointStatusResponse>,
}

impl From<Organization> for OrganizationResponse {
    fn from(value: Organization) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            description: value.description,
            organization_type: value.organization_type,
            parent_organization_id: value.parent_organization_id.map(|id| id.to_string()),
            enabled: value.enabled,
            properties: value.properties,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<IntegrationSystem> for IntegrationSystemResponse {
    fn from(value: IntegrationSystem) -> Self {
        Self {
            id: value.id.to_string(),
            organization_id: value.organization_id.to_string(),
            name: value.name,
            description: value.description,
            system_type: value.system_type,
            topic_namespace: value.topic_namespace,
            topic_prefix: value.topic_prefix,
            enabled: value.enabled,
            properties: value.properties,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// 端点健康响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealthResponse {
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub circuit_state: Option<String>,
    pub consecutive_failures: u32,
    pub last_delivery_at: Option<chrono::DateTime<chrono::Utc>>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

/// 端点状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStatusResponse {
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    pub circuit_state: Option<String>,
    pub consecutive_failures: u32,
    pub last_check_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_delivery_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 端点版本响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointVersionResponse {
    pub version: u32,
    pub changed_at: chrono::DateTime<chrono::Utc>,
    pub changed_by: Option<String>,
    pub change_note: Option<String>,
    pub snapshot: EndpointResponse,
}

// ============ 消息相关 ============

/// 消息查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueryParams {
    pub source_system: Option<String>,
    pub target_system: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    pub source_system: String,
    pub target_system: Option<String>,
    pub protocol: String,
    pub message_type: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub processed_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ============ 死信队列相关 ============

/// DLQ 查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqQueryParams {
    pub reason: Option<String>,
    pub source_system: Option<String>,
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// DLQ 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqMessageResponse {
    pub id: String,
    pub message_id: String,
    pub reason: String,
    pub error_detail: String,
    pub retry_count: u32,
    pub source_system: String,
    pub target_system: Option<String>,
    pub dead_lettered_at: chrono::DateTime<chrono::Utc>,
}

// ============ 审计相关 ============

/// 审计查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQueryParams {
    pub message_id: Option<String>,
    pub trace_id: Option<String>,
    pub event_type: Option<String>,
    pub source_system: Option<String>,
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    pub failed_only: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 审计事件响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventResponse {
    pub id: String,
    pub trace_id: Option<String>,
    pub message_id: Option<String>,
    pub event_type: String,
    pub severity: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_system: Option<String>,
    pub target_system: Option<String>,
    pub component: String,
    pub description: String,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

// ============ 系统状态 ============

/// 系统状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub components: HashMap<String, ComponentStatus>,
    pub stats: SystemStats,
}

/// 组件状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub name: String,
    pub status: String,
    pub message: Option<String>,
}

/// 系统统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub messages_received: u64,
    pub messages_sent: u64,
    pub messages_failed: u64,
    pub active_routes: usize,
    pub active_endpoints: usize,
    pub dlq_size: usize,
    pub queue_size: usize,
}

/// 熔断器状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerResponse {
    pub name: String,
    pub state: String,
    pub failure_count: u32,
    pub success_count: u32,
    pub last_failure: Option<chrono::DateTime<chrono::Utc>>,
}

// ============ 工作流相关 ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepPayload {
    pub id: String,
    pub name: String,
    pub step_type: StepType,
    pub config: Option<StepConfig>,
    pub retry: Option<RetryPolicy>,
    pub timeout_ms: Option<u64>,
    pub condition: Option<String>,
    pub compensation_step: Option<Box<WorkflowStepPayload>>,
    pub next_steps: Option<Vec<NextStep>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCompensationPolicyPayload {
    pub mode: CompensationMode,
    pub timeout_ms: u64,
    pub continue_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflowRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<u32>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub compensation: Option<WorkflowCompensationPolicyPayload>,
    pub options: Option<WorkflowOptions>,
    pub steps: Vec<WorkflowStepPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<u32>,
    pub enabled: Option<bool>,
    pub timeout_ms: Option<u64>,
    pub compensation: Option<WorkflowCompensationPolicyPayload>,
    pub clear_compensation: Option<bool>,
    pub options: Option<WorkflowOptions>,
    pub steps: Option<Vec<WorkflowStepPayload>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: u32,
    pub enabled: bool,
    pub timeout_ms: u64,
    pub compensation: Option<WorkflowCompensationPolicyPayload>,
    pub options: WorkflowOptions,
    pub steps: Vec<WorkflowStepPayload>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstanceQueryParams {
    pub workflow_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkflowInstanceRequest {
    pub source_system: String,
    pub target_system: Option<String>,
    pub protocol: ProtocolType,
    pub message_type: Option<String>,
    pub correlation_id: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub raw_payload_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstanceResponse {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    pub status: String,
    pub current_step_id: Option<String>,
    pub context: WorkflowContext,
    pub step_history: Vec<StepExecution>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
}
