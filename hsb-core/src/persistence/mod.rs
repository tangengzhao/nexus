//! HSB 持久化层
//!
//! 双层持久化架构：
//! - **Redb**（本地层）：嵌入式 KV 存储，用于本地缓存、WAL、幂等去重、熔断器状态
//! - **PostgreSQL**（持久层）：关系型存储，用于消息归档、路由规则持久化、审计合规、DLQ

pub mod pg_store;
pub mod redb_store;

pub use pg_store::PgStore;
pub use redb_store::RedbStore;

use crate::workflow::WorkflowInstance;
use crate::{
    Endpoint, EndpointRuntimeStatus, EndpointVersionRecord, IntegrationSystem, Message,
    Organization, Workflow,
};
use async_trait::async_trait;
use hsb_common::HsbResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 消息查询条件
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistentMessageQuery {
    pub source_system: Option<String>,
    pub target_system: Option<String>,
    pub message_type: Option<String>,
    pub status: Option<String>,
    pub from_time: Option<chrono::DateTime<chrono::Utc>>,
    pub to_time: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 持久化消息存储 Trait
#[async_trait]
pub trait PersistentMessageStore: Send + Sync {
    /// 持久化保存消息
    async fn save_message(&self, msg: &Message) -> HsbResult<()>;

    /// 查询消息列表
    async fn list_messages(&self, query: &PersistentMessageQuery) -> HsbResult<Vec<Message>>;

    /// 根据 ID 获取消息
    async fn get_message(&self, id: &str) -> HsbResult<Option<Message>>;

    /// 删除消息
    async fn delete_message(&self, id: &str) -> HsbResult<()>;

    /// 查询待处理消息（用于恢复）
    async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>>;

    /// 批量保存消息
    async fn save_batch(&self, messages: &[Message]) -> HsbResult<()>;
}

/// 幂等性存储 Trait（用于 ExactlyOnce 语义）
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// 检查并标记消息 ID（返回 true 表示是新消息）
    async fn check_and_mark(&self, idempotency_key: &str, ttl_secs: u64) -> HsbResult<bool>;

    /// 检查消息是否已处理
    async fn is_processed(&self, idempotency_key: &str) -> HsbResult<bool>;

    /// 清理指定幂等键（用于发送失败后回滚占位）
    async fn clear_mark(&self, idempotency_key: &str) -> HsbResult<()>;

    /// 清理过期记录
    async fn cleanup_expired(&self) -> HsbResult<u64>;
}

/// 路由持久化存储 Trait
#[async_trait]
pub trait RouteStore: Send + Sync {
    /// 保存路由规则
    async fn save_route(&self, route: &crate::Route) -> HsbResult<()>;

    /// 获取路由规则
    async fn get_route(&self, id: &str) -> HsbResult<Option<crate::Route>>;

    /// 获取所有路由规则
    async fn list_routes(&self) -> HsbResult<Vec<crate::Route>>;

    /// 删除路由规则
    async fn delete_route(&self, id: &str) -> HsbResult<()>;
}

/// 端点持久化存储 Trait
#[async_trait]
pub trait EndpointStore: Send + Sync {
    /// 创建端点
    async fn create_endpoint(
        &self,
        endpoint: &Endpoint,
        actor: Option<&str>,
        change_note: Option<&str>,
    ) -> HsbResult<()>;

    /// 获取端点
    async fn get_endpoint(&self, id: &str) -> HsbResult<Option<Endpoint>>;

    /// 列出端点
    async fn list_endpoints(&self) -> HsbResult<Vec<Endpoint>>;

    /// 更新端点
    async fn update_endpoint(
        &self,
        endpoint: &Endpoint,
        actor: Option<&str>,
        change_note: Option<&str>,
    ) -> HsbResult<()>;

    /// 删除端点
    async fn delete_endpoint(&self, id: &str) -> HsbResult<()>;

    /// 获取端点版本历史
    async fn list_endpoint_versions(&self, id: &str) -> HsbResult<Vec<EndpointVersionRecord>>;

    /// 获取端点运行状态
    async fn get_endpoint_status(&self, id: &str) -> HsbResult<Option<EndpointRuntimeStatus>>;

    /// 更新端点运行状态
    async fn upsert_endpoint_status(&self, status: &EndpointRuntimeStatus) -> HsbResult<()>;
}

/// 机构持久化存储 Trait
#[async_trait]
pub trait OrganizationStore: Send + Sync {
    async fn create_organization(&self, organization: &Organization) -> HsbResult<()>;
    async fn get_organization(&self, id: &str) -> HsbResult<Option<Organization>>;
    async fn list_organizations(&self) -> HsbResult<Vec<Organization>>;
    async fn update_organization(&self, organization: &Organization) -> HsbResult<()>;
    async fn delete_organization(&self, id: &str) -> HsbResult<()>;
}

/// 集成系统持久化存储 Trait
#[async_trait]
pub trait IntegrationSystemStore: Send + Sync {
    async fn create_system(&self, system: &IntegrationSystem) -> HsbResult<()>;
    async fn get_system(&self, id: &str) -> HsbResult<Option<IntegrationSystem>>;
    async fn list_systems(&self) -> HsbResult<Vec<IntegrationSystem>>;
    async fn update_system(&self, system: &IntegrationSystem) -> HsbResult<()>;
    async fn delete_system(&self, id: &str) -> HsbResult<()>;
}

/// 持久化的自定义协议定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCustomProtocol {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub transport_hint: Option<String>,
    pub content_type: Option<String>,
    pub fields: serde_json::Value,
    pub sample_payload: Option<serde_json::Value>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 自定义协议持久化存储 Trait
#[async_trait]
pub trait CustomProtocolStore: Send + Sync {
    async fn create_custom_protocol(&self, protocol: &StoredCustomProtocol) -> HsbResult<()>;
    async fn get_custom_protocol(&self, id: &str) -> HsbResult<Option<StoredCustomProtocol>>;
    async fn list_custom_protocols(&self) -> HsbResult<Vec<StoredCustomProtocol>>;
    async fn update_custom_protocol(&self, protocol: &StoredCustomProtocol) -> HsbResult<()>;
    async fn delete_custom_protocol(&self, id: &str) -> HsbResult<()>;
}

/// 持久化的 Topic 目录项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTopic {
    pub topic: String,
    pub description: Option<String>,
    pub owner_system_id: Option<String>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Topic 目录持久化存储 Trait
#[async_trait]
pub trait TopicStore: Send + Sync {
    async fn create_topic(&self, topic: &StoredTopic) -> HsbResult<()>;
    async fn get_topic(&self, id: &str) -> HsbResult<Option<StoredTopic>>;
    async fn list_topics(&self) -> HsbResult<Vec<StoredTopic>>;
    async fn update_topic(&self, id: &str, topic: &StoredTopic) -> HsbResult<()>;
    async fn delete_topic(&self, id: &str) -> HsbResult<()>;
}

/// 工作流实例查询条件
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowInstanceQuery {
    pub workflow_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// 持久化的工作流定义记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredWorkflowDefinition {
    pub workflow: Workflow,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// 工作流定义/实例持久化 Trait
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    /// 保存工作流定义
    async fn save_workflow(&self, workflow: &Workflow) -> HsbResult<()>;

    /// 获取工作流定义
    async fn get_workflow(&self, id: &str) -> HsbResult<Option<StoredWorkflowDefinition>>;

    /// 列出工作流定义
    async fn list_workflows(&self) -> HsbResult<Vec<StoredWorkflowDefinition>>;

    /// 删除工作流定义
    async fn delete_workflow(&self, id: &str) -> HsbResult<()>;

    /// 保存工作流实例快照
    async fn save_workflow_instance(&self, instance: &WorkflowInstance) -> HsbResult<()>;

    /// 获取工作流实例
    async fn get_workflow_instance(&self, id: &str) -> HsbResult<Option<WorkflowInstance>>;

    /// 列出工作流实例
    async fn list_workflow_instances(
        &self,
        query: &WorkflowInstanceQuery,
    ) -> HsbResult<Vec<WorkflowInstance>>;

    /// 删除工作流实例
    async fn delete_workflow_instance(&self, id: &str) -> HsbResult<()>;
}

/// 持久化层配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Redb 配置
    pub redb: RedbConfig,
    /// PostgreSQL 配置
    pub postgres: PostgresConfig,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            redb: RedbConfig::default(),
            postgres: PostgresConfig::default(),
        }
    }
}

/// Redb 本地存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedbConfig {
    /// 数据库文件路径
    pub path: String,
    /// 是否启用
    pub enabled: bool,
}

impl Default for RedbConfig {
    fn default() -> Self {
        Self {
            path: "data/hsb_local.redb".to_string(),
            enabled: true,
        }
    }
}

/// PostgreSQL 持久化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    /// 数据库 URL
    pub url: String,
    /// 最大连接数
    pub max_connections: u32,
    /// 最小连接数
    pub min_connections: u32,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 是否启用
    pub enabled: bool,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            url: std::env::var("HSB_DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@postgres:5432/hsb".to_string()),
            max_connections: 10,
            min_connections: 2,
            connect_timeout_secs: 30,
            enabled: true,
        }
    }
}
