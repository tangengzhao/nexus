//! 管理 API 状态

use crate::audit::{AuditEvent, AuditFilter, AuditService, MessageTrace};
use chrono::{DateTime, Utc};
use hsb_common::{
    EndpointRole, HsbError, HsbResult, OrganizationId, ProtocolType, SystemId, Topic,
};
use hsb_core::engine::{EndpointInfo, EndpointRegistry, Router};
use hsb_core::persistence::{
    CustomProtocolStore, EndpointStore, IntegrationSystemStore, OrganizationStore,
    PersistentMessageQuery, PersistentMessageStore, RouteStore, StoredCustomProtocol, StoredTopic,
    StoredWorkflowDefinition, TopicStore, WorkflowInstanceQuery, WorkflowStore,
};
use hsb_core::reliability::{CircuitBreakerRegistry, DeadLetter, DeadLetterQueue, DeadLetterStats};
use hsb_core::workflow::{
    CompensationPolicy, Workflow, WorkflowExecutor, WorkflowInstance, WorkflowOptions, WorkflowStep,
};
use hsb_core::{
    AuthConfig, DeliveryMode, Endpoint, EndpointLifecycleStatus, EndpointRuntimeStatus,
    EndpointSecurity, IntegrationSystem, Message, MessageBuilder, Organization, Route,
    RouteOptions, RouteTarget, SourceMatch,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::models::*;

const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 500;

#[async_trait::async_trait]
pub trait MessageReplayService: Send + Sync {
    async fn replay(&self, message: Message) -> HsbResult<()>;
}

#[derive(Clone)]
struct WorkflowDefinitionRecord {
    workflow: Workflow,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
struct CustomProtocolRecord {
    item: CustomProtocolResponse,
}

#[derive(Clone)]
struct TopicRecord {
    item: TopicResponse,
}

/// 管理 API 状态
pub struct AdminState {
    // 路由管理
    router: Arc<dyn Router>,
    // 路由持久化存储
    route_store: Option<Arc<dyn RouteStore>>,
    // 端点注册表
    endpoints: Arc<RwLock<EndpointRegistry>>,
    // 端点持久化存储
    endpoint_store: Option<Arc<dyn EndpointStore>>,
    // 机构目录持久化
    organization_store: Option<Arc<dyn OrganizationStore>>,
    // 系统目录持久化
    system_store: Option<Arc<dyn IntegrationSystemStore>>,
    // 自定义协议持久化
    custom_protocol_store: Option<Arc<dyn CustomProtocolStore>>,
    // Topic 目录持久化
    topic_store: Option<Arc<dyn TopicStore>>,
    // 机构目录
    organizations: Arc<RwLock<HashMap<String, Organization>>>,
    // 系统目录
    systems: Arc<RwLock<HashMap<String, IntegrationSystem>>>,
    // 消息持久化存储
    message_store: Option<Arc<dyn PersistentMessageStore>>,
    // 死信队列
    dlq: Arc<dyn DeadLetterQueue>,
    // 消息重放服务
    message_replay: Option<Arc<dyn MessageReplayService>>,
    // 审计服务
    audit: Arc<dyn AuditService>,
    // 熔断器注册表
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    // 工作流定义持久化
    workflow_store: Option<Arc<dyn WorkflowStore>>,
    // 工作流执行器
    workflow_executor: Option<Arc<dyn WorkflowExecutor>>,
    // 工作流定义
    workflow_definitions: Arc<RwLock<HashMap<String, WorkflowDefinitionRecord>>>,
    // 自定义协议定义
    custom_protocols: Arc<RwLock<HashMap<String, CustomProtocolRecord>>>,
    // Topic 目录
    topics: Arc<RwLock<HashMap<String, TopicRecord>>>,
    // 配置
    config: Arc<RwLock<serde_json::Value>>,
    // 启动时间
    start_time: Instant,
}

impl AdminState {
    pub fn new(
        router: Arc<dyn Router>,
        route_store: Option<Arc<dyn RouteStore>>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
        endpoint_store: Option<Arc<dyn EndpointStore>>,
        organization_store: Option<Arc<dyn OrganizationStore>>,
        system_store: Option<Arc<dyn IntegrationSystemStore>>,
        custom_protocol_store: Option<Arc<dyn CustomProtocolStore>>,
        topic_store: Option<Arc<dyn TopicStore>>,
        message_store: Option<Arc<dyn PersistentMessageStore>>,
        workflow_store: Option<Arc<dyn WorkflowStore>>,
        dlq: Arc<dyn DeadLetterQueue>,
        message_replay: Option<Arc<dyn MessageReplayService>>,
        audit: Arc<dyn AuditService>,
        circuit_breakers: Arc<CircuitBreakerRegistry>,
        workflow_executor: Option<Arc<dyn WorkflowExecutor>>,
    ) -> Self {
        Self {
            router,
            route_store,
            endpoints,
            endpoint_store,
            organization_store,
            system_store,
            custom_protocol_store,
            topic_store,
            organizations: Arc::new(RwLock::new(HashMap::new())),
            systems: Arc::new(RwLock::new(HashMap::new())),
            message_store,
            workflow_store,
            dlq,
            message_replay,
            audit,
            circuit_breakers,
            workflow_executor,
            workflow_definitions: Arc::new(RwLock::new(HashMap::new())),
            custom_protocols: Arc::new(RwLock::new(HashMap::new())),
            topics: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(serde_json::json!({}))),
            start_time: Instant::now(),
        }
    }

    // ============ 自定义协议维护 ============

    pub async fn list_custom_protocols(&self) -> HsbResult<Vec<CustomProtocolResponse>> {
        if let Some(store) = &self.custom_protocol_store {
            return store
                .list_custom_protocols()
                .await?
                .into_iter()
                .map(stored_custom_protocol_to_response)
                .collect();
        }

        let mut items: Vec<_> = self
            .custom_protocols
            .read()
            .await
            .values()
            .map(|record| record.item.clone())
            .collect();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(items)
    }

    fn bounded_limit(limit: Option<usize>) -> usize {
        limit.unwrap_or(DEFAULT_LIST_LIMIT).min(MAX_LIST_LIMIT)
    }

    pub async fn get_custom_protocol(&self, id: &str) -> HsbResult<Option<CustomProtocolResponse>> {
        if let Some(store) = &self.custom_protocol_store {
            return store
                .get_custom_protocol(id)
                .await?
                .map(stored_custom_protocol_to_response)
                .transpose();
        }

        Ok(self
            .custom_protocols
            .read()
            .await
            .get(id)
            .map(|record| record.item.clone()))
    }

    pub async fn create_custom_protocol(
        &self,
        req: CreateCustomProtocolRequest,
    ) -> HsbResult<CustomProtocolResponse> {
        validate_custom_protocol_fields(&req.fields)?;
        let id = req
            .id
            .unwrap_or_else(|| format!("custom_{}", ulid::Ulid::new()));
        if self.get_custom_protocol(&id).await?.is_some() {
            return Err(HsbError::DuplicateRecord {
                entity: "CustomProtocol".to_string(),
                id,
            });
        }

        let now = Utc::now();
        let item = CustomProtocolResponse {
            id: id.clone(),
            name: req.name,
            description: req.description,
            transport_hint: req.transport_hint,
            content_type: req.content_type,
            fields: req.fields,
            sample_payload: req.sample_payload,
            enabled: req.enabled.unwrap_or(true),
            created_at: now,
            updated_at: now,
        };

        if let Some(store) = &self.custom_protocol_store {
            store
                .create_custom_protocol(&custom_protocol_response_to_stored(&item)?)
                .await?;
            return Ok(item);
        }

        let mut protocols = self.custom_protocols.write().await;
        protocols.insert(id.clone(), CustomProtocolRecord { item: item.clone() });
        Ok(item)
    }

    pub async fn update_custom_protocol(
        &self,
        id: &str,
        req: UpdateCustomProtocolRequest,
    ) -> HsbResult<CustomProtocolResponse> {
        if let Some(store) = &self.custom_protocol_store {
            let mut item = store
                .get_custom_protocol(id)
                .await?
                .map(stored_custom_protocol_to_response)
                .transpose()?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "CustomProtocol".to_string(),
                    id: id.to_string(),
                })?;
            if let Some(name) = req.name {
                item.name = name;
            }
            if let Some(description) = req.description {
                item.description = Some(description);
            }
            if let Some(transport_hint) = req.transport_hint {
                item.transport_hint = Some(transport_hint);
            }
            if let Some(content_type) = req.content_type {
                item.content_type = Some(content_type);
            }
            if let Some(fields) = req.fields {
                validate_custom_protocol_fields(&fields)?;
                item.fields = fields;
            }
            if let Some(sample_payload) = req.sample_payload {
                item.sample_payload = Some(sample_payload);
            }
            if let Some(enabled) = req.enabled {
                item.enabled = enabled;
            }
            item.updated_at = Utc::now();
            store
                .update_custom_protocol(&custom_protocol_response_to_stored(&item)?)
                .await?;
            return Ok(item);
        }

        let mut protocols = self.custom_protocols.write().await;
        let record = protocols.get_mut(id).ok_or_else(|| HsbError::NotFound {
            entity: "CustomProtocol".to_string(),
            id: id.to_string(),
        })?;
        if let Some(name) = req.name {
            record.item.name = name;
        }
        if let Some(description) = req.description {
            record.item.description = Some(description);
        }
        if let Some(transport_hint) = req.transport_hint {
            record.item.transport_hint = Some(transport_hint);
        }
        if let Some(content_type) = req.content_type {
            record.item.content_type = Some(content_type);
        }
        if let Some(fields) = req.fields {
            validate_custom_protocol_fields(&fields)?;
            record.item.fields = fields;
        }
        if let Some(sample_payload) = req.sample_payload {
            record.item.sample_payload = Some(sample_payload);
        }
        if let Some(enabled) = req.enabled {
            record.item.enabled = enabled;
        }
        record.item.updated_at = Utc::now();
        Ok(record.item.clone())
    }

    pub async fn delete_custom_protocol(&self, id: &str) -> HsbResult<()> {
        if let Some(store) = &self.custom_protocol_store {
            if store.get_custom_protocol(id).await?.is_none() {
                return Err(HsbError::NotFound {
                    entity: "CustomProtocol".to_string(),
                    id: id.to_string(),
                });
            }
            store.delete_custom_protocol(id).await?;
            return Ok(());
        }

        let removed = self.custom_protocols.write().await.remove(id);
        if removed.is_none() {
            return Err(HsbError::NotFound {
                entity: "CustomProtocol".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // ============ Topic 维护 ============

    pub async fn list_topics(&self) -> HsbResult<Vec<TopicResponse>> {
        if let Some(store) = &self.topic_store {
            return store
                .list_topics()
                .await?
                .into_iter()
                .map(stored_topic_to_response)
                .collect();
        }

        let mut items: Vec<_> = self
            .topics
            .read()
            .await
            .values()
            .map(|record| record.item.clone())
            .collect();
        items.sort_by(|left, right| left.topic.cmp(&right.topic));
        Ok(items)
    }

    pub async fn get_topic(&self, id: &str) -> HsbResult<Option<TopicResponse>> {
        if let Some(store) = &self.topic_store {
            return store
                .get_topic(id)
                .await?
                .map(stored_topic_to_response)
                .transpose();
        }

        Ok(self
            .topics
            .read()
            .await
            .get(id)
            .map(|record| record.item.clone()))
    }

    pub async fn create_topic(&self, req: CreateTopicRequest) -> HsbResult<TopicResponse> {
        let topic = parse_topic_response(
            req.topic,
            req.description,
            req.owner_system_id,
            req.enabled.unwrap_or(true),
            req.properties.unwrap_or_default(),
            Utc::now(),
            Utc::now(),
        )?;
        if self.get_topic(&topic.id).await?.is_some() {
            return Err(HsbError::DuplicateRecord {
                entity: "Topic".to_string(),
                id: topic.id.clone(),
            });
        }

        if let Some(store) = &self.topic_store {
            store
                .create_topic(&topic_response_to_stored(&topic))
                .await?;
            return Ok(topic);
        }

        let mut topics = self.topics.write().await;
        topics.insert(
            topic.id.clone(),
            TopicRecord {
                item: topic.clone(),
            },
        );
        Ok(topic)
    }

    pub async fn update_topic(
        &self,
        id: &str,
        req: UpdateTopicRequest,
    ) -> HsbResult<TopicResponse> {
        if let Some(store) = &self.topic_store {
            let current = store
                .get_topic(id)
                .await?
                .map(stored_topic_to_response)
                .transpose()?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Topic".to_string(),
                    id: id.to_string(),
                })?;
            let topic_name = req.topic.unwrap_or_else(|| current.topic.clone());
            let updated = parse_topic_response(
                topic_name,
                req.description.or(current.description),
                req.owner_system_id.or(current.owner_system_id),
                req.enabled.unwrap_or(current.enabled),
                req.properties.unwrap_or(current.properties),
                current.created_at,
                Utc::now(),
            )?;

            if updated.id != id && self.get_topic(&updated.id).await?.is_some() {
                return Err(HsbError::DuplicateRecord {
                    entity: "Topic".to_string(),
                    id: updated.id.clone(),
                });
            }
            store
                .update_topic(id, &topic_response_to_stored(&updated))
                .await?;
            return Ok(updated);
        }

        let mut topics = self.topics.write().await;
        let current = topics
            .get(id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "Topic".to_string(),
                id: id.to_string(),
            })?
            .item
            .clone();
        let topic_name = req.topic.unwrap_or_else(|| current.topic.clone());
        let updated = parse_topic_response(
            topic_name,
            req.description.or(current.description),
            req.owner_system_id.or(current.owner_system_id),
            req.enabled.unwrap_or(current.enabled),
            req.properties.unwrap_or(current.properties),
            current.created_at,
            Utc::now(),
        )?;
        if updated.id != id && topics.contains_key(&updated.id) {
            return Err(HsbError::DuplicateRecord {
                entity: "Topic".to_string(),
                id: updated.id.clone(),
            });
        }
        topics.remove(id);
        topics.insert(
            updated.id.clone(),
            TopicRecord {
                item: updated.clone(),
            },
        );
        Ok(updated)
    }

    pub async fn delete_topic(&self, id: &str) -> HsbResult<()> {
        if let Some(store) = &self.topic_store {
            if store.get_topic(id).await?.is_none() {
                return Err(HsbError::NotFound {
                    entity: "Topic".to_string(),
                    id: id.to_string(),
                });
            }
            store.delete_topic(id).await?;
            return Ok(());
        }

        let removed = self.topics.write().await.remove(id);
        if removed.is_none() {
            return Err(HsbError::NotFound {
                entity: "Topic".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // ============ 就绪检查 ============

    pub async fn is_ready(&self) -> bool {
        // 检查所有组件是否就绪
        true
    }

    pub async fn readiness_checks(&self) -> HashMap<String, CheckResult> {
        let mut checks = HashMap::new();

        checks.insert(
            "router".to_string(),
            CheckResult {
                status: "ok".to_string(),
                message: None,
            },
        );

        checks.insert(
            "dlq".to_string(),
            CheckResult {
                status: "ok".to_string(),
                message: None,
            },
        );

        checks
    }

    // ============ 系统状态 ============

    pub async fn system_status(&self) -> SystemStatusResponse {
        let routes = self.router.list_routes().await.unwrap_or_default();
        let endpoints = self.endpoints.read().await;
        let dlq_stats = self.dlq.stats().await.unwrap_or_default();

        SystemStatusResponse {
            status: "running".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.start_time.elapsed().as_secs(),
            components: HashMap::new(),
            stats: SystemStats {
                messages_received: 0,
                messages_sent: 0,
                messages_failed: 0,
                active_routes: routes.len(),
                active_endpoints: endpoints.list().len(),
                dlq_size: dlq_stats.total as usize,
                queue_size: 0,
            },
        }
    }

    pub async fn get_metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "uptime_secs": self.start_time.elapsed().as_secs(),
        })
    }

    // ============ 机构/系统目录 ============

    pub async fn list_organizations(&self) -> HsbResult<Vec<OrganizationResponse>> {
        if let Some(store) = &self.organization_store {
            let items = store.list_organizations().await?;
            return Ok(items.into_iter().map(Into::into).collect());
        }

        let mut items: Vec<_> = self.organizations.read().await.values().cloned().collect();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(items.into_iter().map(Into::into).collect())
    }

    pub async fn get_organization(&self, id: &str) -> HsbResult<Option<OrganizationResponse>> {
        if let Some(store) = &self.organization_store {
            return Ok(store.get_organization(id).await?.map(Into::into));
        }

        Ok(self
            .organizations
            .read()
            .await
            .get(id)
            .cloned()
            .map(Into::into))
    }

    pub async fn create_organization(
        &self,
        req: CreateOrganizationRequest,
    ) -> HsbResult<OrganizationResponse> {
        let id = req
            .id
            .unwrap_or_else(|| format!("org_{}", ulid::Ulid::new()));
        if self.get_organization(&id).await?.is_some() {
            return Err(HsbError::DuplicateRecord {
                entity: "Organization".to_string(),
                id,
            });
        }

        let mut organization = Organization::new(id.clone(), req.name, req.organization_type);
        organization.description = req.description;
        organization.parent_organization_id = req.parent_organization_id.map(OrganizationId::new);
        organization.enabled = req.enabled.unwrap_or(true);
        organization.properties = req.properties.unwrap_or_default();

        if let Some(parent_id) = &organization.parent_organization_id {
            self.load_organization(parent_id.as_str()).await?;
        }

        if let Some(store) = &self.organization_store {
            store.create_organization(&organization).await?;
        } else {
            self.organizations
                .write()
                .await
                .insert(id.clone(), organization.clone());
        }

        Ok(organization.into())
    }

    pub async fn update_organization(
        &self,
        id: &str,
        req: UpdateOrganizationRequest,
    ) -> HsbResult<OrganizationResponse> {
        let mut organization = self.load_organization(id).await?;
        if let Some(name) = req.name {
            organization.name = name;
        }
        if let Some(description) = req.description {
            organization.description = Some(description);
        }
        if let Some(organization_type) = req.organization_type {
            organization.organization_type = organization_type;
        }
        if let Some(parent_organization_id) = req.parent_organization_id {
            if parent_organization_id != id {
                self.load_organization(&parent_organization_id).await?;
            }
            organization.parent_organization_id = Some(OrganizationId::new(parent_organization_id));
        }
        if let Some(enabled) = req.enabled {
            organization.enabled = enabled;
        }
        if let Some(properties) = req.properties {
            organization.properties = properties;
        }
        organization.updated_at = Utc::now();

        if let Some(store) = &self.organization_store {
            store.update_organization(&organization).await?;
        } else {
            self.organizations
                .write()
                .await
                .insert(id.to_string(), organization.clone());
        }

        Ok(organization.into())
    }

    pub async fn delete_organization(&self, id: &str) -> HsbResult<()> {
        let systems = self.list_systems().await?;
        if systems.iter().any(|item| item.organization_id == id) {
            return Err(HsbError::ValidationError {
                message: format!("Organization {} still owns systems", id),
            });
        }

        if let Some(store) = &self.organization_store {
            store.delete_organization(id).await?;
        } else {
            self.organizations.write().await.remove(id);
        }
        Ok(())
    }

    pub async fn list_systems(&self) -> HsbResult<Vec<IntegrationSystemResponse>> {
        if let Some(store) = &self.system_store {
            let items = store.list_systems().await?;
            return Ok(items.into_iter().map(Into::into).collect());
        }

        let mut items: Vec<_> = self.systems.read().await.values().cloned().collect();
        items.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(items.into_iter().map(Into::into).collect())
    }

    pub async fn get_system(&self, id: &str) -> HsbResult<Option<IntegrationSystemResponse>> {
        if let Some(store) = &self.system_store {
            return Ok(store.get_system(id).await?.map(Into::into));
        }

        Ok(self.systems.read().await.get(id).cloned().map(Into::into))
    }

    pub async fn create_system(
        &self,
        req: CreateIntegrationSystemRequest,
    ) -> HsbResult<IntegrationSystemResponse> {
        self.load_organization(&req.organization_id).await?;
        let id = req
            .id
            .unwrap_or_else(|| format!("sys_{}", ulid::Ulid::new()));
        if self.get_system(&id).await?.is_some() {
            return Err(HsbError::DuplicateRecord {
                entity: "IntegrationSystem".to_string(),
                id,
            });
        }

        let mut system =
            IntegrationSystem::new(id.clone(), req.organization_id, req.name, req.system_type);
        system.description = req.description;
        system.topic_namespace = req.topic_namespace;
        system.topic_prefix = req.topic_prefix;
        system.enabled = req.enabled.unwrap_or(true);
        system.properties = req.properties.unwrap_or_default();

        if let Some(store) = &self.system_store {
            store.create_system(&system).await?;
        } else {
            self.systems
                .write()
                .await
                .insert(id.clone(), system.clone());
        }

        Ok(system.into())
    }

    pub async fn update_system(
        &self,
        id: &str,
        req: UpdateIntegrationSystemRequest,
    ) -> HsbResult<IntegrationSystemResponse> {
        let mut system = self.load_system(id).await?;
        if let Some(organization_id) = req.organization_id {
            self.load_organization(&organization_id).await?;
            system.organization_id = OrganizationId::new(organization_id);
        }
        if let Some(name) = req.name {
            system.name = name;
        }
        if let Some(description) = req.description {
            system.description = Some(description);
        }
        if let Some(system_type) = req.system_type {
            system.system_type = system_type;
        }
        if let Some(topic_namespace) = req.topic_namespace {
            system.topic_namespace = Some(topic_namespace);
        }
        if let Some(topic_prefix) = req.topic_prefix {
            system.topic_prefix = Some(topic_prefix);
        }
        if let Some(enabled) = req.enabled {
            system.enabled = enabled;
        }
        if let Some(properties) = req.properties {
            system.properties = properties;
        }
        system.updated_at = Utc::now();

        if let Some(store) = &self.system_store {
            store.update_system(&system).await?;
        } else {
            self.systems
                .write()
                .await
                .insert(id.to_string(), system.clone());
        }

        Ok(system.into())
    }

    pub async fn delete_system(&self, id: &str) -> HsbResult<()> {
        if let Some(store) = &self.endpoint_store {
            let endpoints = store.list_endpoints().await?;
            if endpoints
                .iter()
                .any(|endpoint| endpoint.system_id.as_str() == id)
            {
                return Err(HsbError::ValidationError {
                    message: format!("System {} still owns endpoints", id),
                });
            }
        }

        if let Some(store) = &self.system_store {
            store.delete_system(id).await?;
        } else {
            self.systems.write().await.remove(id);
        }
        Ok(())
    }

    // ============ 路由管理 ============

    pub async fn list_routes(&self) -> HsbResult<Vec<RouteResponse>> {
        let routes = if let Some(store) = &self.route_store {
            store.list_routes().await?
        } else {
            self.router.list_routes().await?
        };
        Ok(routes.into_iter().map(|r| route_to_response(&r)).collect())
    }

    pub async fn get_route(&self, id: &str) -> HsbResult<Option<RouteResponse>> {
        if let Some(store) = &self.route_store {
            return Ok(store
                .get_route(id)
                .await?
                .map(|route| route_to_response(&route)));
        }

        let routes = self.router.list_routes().await?;
        Ok(routes
            .into_iter()
            .find(|r| r.id.to_string() == id)
            .map(|r| route_to_response(&r)))
    }

    pub async fn create_route(&self, req: CreateRouteRequest) -> HsbResult<RouteResponse> {
        let route = build_route_from_create(req)?;
        self.router.add_route(route.clone()).await?;
        if let Some(store) = &self.route_store {
            store.save_route(&route).await?;
        }
        Ok(route_to_response(&route))
    }

    pub async fn update_route(
        &self,
        id: &str,
        req: UpdateRouteRequest,
    ) -> HsbResult<RouteResponse> {
        let mut route = self.load_route(id).await?;
        let req_for_options = req.clone();

        let source_system = req.source_system.clone();
        let protocol = req.protocol;
        let message_type = req.message_type.clone();

        if let Some(name) = req.name {
            route.name = name;
        }
        if let Some(description) = req.description {
            route.description = Some(description);
        }
        if source_system.is_some() || protocol.is_some() || message_type.is_some() {
            route.source_match = SourceMatch {
                system_id: source_system.or(route.source_match.system_id.clone()),
                protocol: protocol.or(route.source_match.protocol),
                message_type_pattern: message_type
                    .or(route.source_match.message_type_pattern.clone()),
            };
        }
        if let Some(conditions) = req.conditions {
            route.conditions = conditions;
        }
        if let Some(targets) = req.targets {
            route.targets = build_route_targets(targets)?;
        }
        if let Some(transformer_ids) = req.transformer_ids {
            route.transformer_ids = transformer_ids;
        }
        if let Some(priority) = req.priority {
            route.priority = priority;
        }
        if let Some(enabled) = req.enabled {
            route.enabled = enabled;
        }

        merge_route_options(&mut route.options, &req_for_options);

        self.router.add_route(route.clone()).await?;
        if let Some(store) = &self.route_store {
            store.save_route(&route).await?;
        }

        Ok(route_to_response(&route))
    }

    pub async fn delete_route(&self, id: &str) -> HsbResult<()> {
        self.router.remove_route(id).await?;
        if let Some(store) = &self.route_store {
            store.delete_route(id).await?;
        }
        Ok(())
    }

    pub async fn set_route_enabled(&self, id: &str, enabled: bool) -> HsbResult<()> {
        let mut route = self.load_route(id).await?;
        route.enabled = enabled;
        self.router.add_route(route.clone()).await?;
        if let Some(store) = &self.route_store {
            store.save_route(&route).await?;
        }
        Ok(())
    }

    // ============ 端点管理 ============

    pub async fn list_endpoints(&self) -> HsbResult<Vec<EndpointResponse>> {
        if let Some(store) = &self.endpoint_store {
            let endpoints = store.list_endpoints().await?;
            let mut items = Vec::with_capacity(endpoints.len());
            for endpoint in endpoints {
                let status = self.load_or_initialize_endpoint_status(&endpoint).await?;
                items.push(endpoint_to_response(&endpoint, Some(&status)));
            }
            return Ok(items);
        }

        let endpoints = self.endpoints.read().await;
        Ok(endpoints
            .list()
            .iter()
            .map(|e| endpoint_info_to_response(e))
            .collect())
    }

    pub async fn get_endpoint(&self, id: &str) -> HsbResult<Option<EndpointResponse>> {
        if let Some(store) = &self.endpoint_store {
            let endpoint = store.get_endpoint(id).await?;
            if let Some(endpoint) = endpoint {
                let status = self.load_or_initialize_endpoint_status(&endpoint).await?;
                return Ok(Some(endpoint_to_response(&endpoint, Some(&status))));
            }
            return Ok(None);
        }

        let endpoints = self.endpoints.read().await;
        Ok(endpoints.get(id).map(|e| endpoint_info_to_response(e)))
    }

    pub async fn create_endpoint(&self, req: CreateEndpointRequest) -> HsbResult<EndpointResponse> {
        let system = self.load_system(&req.system_id).await?;
        if req.system_type != system.system_type {
            return Err(HsbError::ValidationError {
                message: format!(
                    "Endpoint system_type {:?} does not match owning system {:?}",
                    req.system_type, system.system_type
                ),
            });
        }
        let roles = req
            .roles
            .clone()
            .unwrap_or_else(|| vec![EndpointRole::Consumer]);
        if roles.is_empty() {
            return Err(HsbError::InvalidField {
                field: "roles".to_string(),
                reason: "At least one endpoint role is required".to_string(),
            });
        }

        let id = req.id.unwrap_or_else(|| ulid::Ulid::new().to_string());
        let mut endpoint = Endpoint::new(
            id.clone(),
            system.organization_id.clone(),
            system.id.clone(),
            req.name,
            system.system_type,
            req.protocol,
            req.connection,
        );
        endpoint.roles = roles;
        endpoint.description = req.description;
        endpoint.auth = req.auth;
        endpoint.config = req.config.unwrap_or_default();
        endpoint.enabled = req.enabled.unwrap_or(true);
        endpoint.lifecycle_status = req.lifecycle_status.unwrap_or_else(|| {
            if endpoint.enabled {
                EndpointLifecycleStatus::Active
            } else {
                EndpointLifecycleStatus::Disabled
            }
        });
        endpoint.security = req.security.unwrap_or_default();
        endpoint.properties = req.properties.unwrap_or_default();
        self.validate_custom_protocol_selection(endpoint.protocol, &endpoint.properties)
            .await?;
        validate_endpoint_protocol_properties(endpoint.protocol, &endpoint.properties)?;
        endpoint.created_by = req.created_by.clone();
        endpoint.updated_by = req.created_by.clone();

        let mut status = EndpointRuntimeStatus::new(endpoint.id.as_str().to_string());
        status.healthy = endpoint.enabled;
        status.last_check_at = Some(chrono::Utc::now());
        status.updated_at = chrono::Utc::now();

        if let Some(store) = &self.endpoint_store {
            store
                .create_endpoint(
                    &endpoint,
                    endpoint.created_by.as_deref(),
                    req.change_note.as_deref(),
                )
                .await?;
            store.upsert_endpoint_status(&status).await?;
        }

        self.sync_endpoint_registry(&endpoint, Some(&status)).await;
        Ok(endpoint_to_response(&endpoint, Some(&status)))
    }

    pub async fn update_endpoint(
        &self,
        id: &str,
        req: UpdateEndpointRequest,
    ) -> HsbResult<EndpointResponse> {
        let store = self.require_endpoint_store()?;
        let mut endpoint = store
            .get_endpoint(id)
            .await?
            .ok_or_else(|| HsbError::NotFound {
                entity: "Endpoint".to_string(),
                id: id.to_string(),
            })?;

        let owning_system = if let Some(system_id) = req.system_id.as_deref() {
            Some(self.load_system(system_id).await?)
        } else {
            None
        };

        if let Some(name) = req.name {
            endpoint.name = name;
        }
        if let Some(description) = req.description {
            endpoint.description = Some(description);
        }
        if let Some(system_type) = req.system_type {
            let expected = owning_system
                .as_ref()
                .map(|value| value.system_type)
                .unwrap_or(endpoint.system_type);
            if system_type != expected {
                return Err(HsbError::ValidationError {
                    message: format!(
                        "Endpoint system_type {:?} does not match owning system {:?}",
                        system_type, expected
                    ),
                });
            }
            endpoint.system_type = system_type;
        }
        if let Some(system) = owning_system {
            endpoint.system_id = system.id.clone();
            endpoint.organization_id = system.organization_id.clone();
            endpoint.system_type = system.system_type;
        }
        if let Some(protocol) = req.protocol {
            endpoint.protocol = protocol;
        }
        if let Some(roles) = req.roles {
            if roles.is_empty() {
                return Err(HsbError::InvalidField {
                    field: "roles".to_string(),
                    reason: "At least one endpoint role is required".to_string(),
                });
            }
            endpoint.roles = roles;
        }
        if let Some(connection) = req.connection {
            endpoint.connection = connection;
        }
        if let Some(auth) = req.auth {
            endpoint.auth = Some(auth);
        }
        if let Some(config) = req.config {
            endpoint.config = config;
        }
        if let Some(enabled) = req.enabled {
            endpoint.enabled = enabled;
        }
        if let Some(lifecycle_status) = req.lifecycle_status {
            endpoint.lifecycle_status = lifecycle_status;
        }
        if let Some(security) = req.security {
            endpoint.security = security;
        }
        if let Some(properties) = req.properties {
            endpoint.properties = properties;
        }
        self.validate_custom_protocol_selection(endpoint.protocol, &endpoint.properties)
            .await?;
        validate_endpoint_protocol_properties(endpoint.protocol, &endpoint.properties)?;
        endpoint.version += 1;
        endpoint.updated_at = chrono::Utc::now();
        endpoint.updated_by = req.updated_by.clone();

        store
            .update_endpoint(
                &endpoint,
                endpoint.updated_by.as_deref(),
                req.change_note.as_deref(),
            )
            .await?;

        let status = self.load_or_initialize_endpoint_status(&endpoint).await?;
        self.sync_endpoint_registry(&endpoint, Some(&status)).await;
        Ok(endpoint_to_response(&endpoint, Some(&status)))
    }

    pub async fn delete_endpoint(&self, id: &str) -> HsbResult<()> {
        if let Some(store) = &self.endpoint_store {
            store.delete_endpoint(id).await?;
        }

        let mut endpoints = self.endpoints.write().await;
        endpoints.remove(id);
        Ok(())
    }

    pub async fn list_endpoint_versions(
        &self,
        id: &str,
    ) -> HsbResult<Vec<EndpointVersionResponse>> {
        let store = self.require_endpoint_store()?;
        let versions = store.list_endpoint_versions(id).await?;
        let mut items = Vec::with_capacity(versions.len());
        for version in versions {
            items.push(EndpointVersionResponse {
                version: version.version,
                changed_at: version.changed_at,
                changed_by: version.changed_by,
                change_note: version.change_note,
                snapshot: endpoint_to_response(&version.snapshot, None),
            });
        }
        Ok(items)
    }

    pub async fn get_endpoint_status(&self, id: &str) -> HsbResult<Option<EndpointStatusResponse>> {
        if let Some(store) = &self.endpoint_store {
            let endpoint = store.get_endpoint(id).await?;
            if let Some(endpoint) = endpoint {
                let status = self.load_or_initialize_endpoint_status(&endpoint).await?;
                return Ok(Some(endpoint_status_to_response(&status)));
            }
            return Ok(None);
        }

        let endpoints = self.endpoints.read().await;
        Ok(endpoints.get(id).map(|endpoint| EndpointStatusResponse {
            healthy: endpoint.healthy,
            latency_ms: None,
            last_error: None,
            circuit_state: None,
            consecutive_failures: 0,
            last_check_at: endpoint.last_heartbeat,
            last_delivery_at: None,
            updated_at: endpoint.last_heartbeat.unwrap_or_else(chrono::Utc::now),
        }))
    }

    pub async fn update_endpoint_status(
        &self,
        id: &str,
        req: UpdateEndpointStatusRequest,
    ) -> HsbResult<EndpointStatusResponse> {
        let store = self.require_endpoint_store()?;
        let endpoint = store
            .get_endpoint(id)
            .await?
            .ok_or_else(|| HsbError::NotFound {
                entity: "Endpoint".to_string(),
                id: id.to_string(),
            })?;

        let mut status = store
            .get_endpoint_status(id)
            .await?
            .unwrap_or_else(|| EndpointRuntimeStatus::new(id.to_string()));
        status.healthy = req.healthy;
        status.latency_ms = req.latency_ms;
        status.last_error = req.last_error;
        status.circuit_state = req.circuit_state;
        status.consecutive_failures = req
            .consecutive_failures
            .unwrap_or(status.consecutive_failures);
        status.last_check_at = req.last_check_at.or_else(|| Some(chrono::Utc::now()));
        status.last_delivery_at = req.last_delivery_at;
        status.updated_at = chrono::Utc::now();

        store.upsert_endpoint_status(&status).await?;
        self.sync_endpoint_registry(&endpoint, Some(&status)).await;
        Ok(endpoint_status_to_response(&status))
    }

    pub async fn update_endpoint_security(
        &self,
        id: &str,
        req: UpdateEndpointSecurityRequest,
    ) -> HsbResult<EndpointResponse> {
        let store = self.require_endpoint_store()?;
        let mut endpoint = store
            .get_endpoint(id)
            .await?
            .ok_or_else(|| HsbError::NotFound {
                entity: "Endpoint".to_string(),
                id: id.to_string(),
            })?;

        merge_security(&mut endpoint.security, &req);
        endpoint.version += 1;
        endpoint.updated_at = chrono::Utc::now();
        endpoint.updated_by = req.rotated_by.clone();

        store
            .update_endpoint(
                &endpoint,
                endpoint.updated_by.as_deref(),
                req.change_note.as_deref(),
            )
            .await?;

        let status = self.load_or_initialize_endpoint_status(&endpoint).await?;
        self.sync_endpoint_registry(&endpoint, Some(&status)).await;
        Ok(endpoint_to_response(&endpoint, Some(&status)))
    }

    pub async fn check_endpoint_health(&self, id: &str) -> HsbResult<EndpointHealthResponse> {
        let status = self
            .get_endpoint_status(id)
            .await?
            .ok_or_else(|| HsbError::NotFound {
                entity: "EndpointStatus".to_string(),
                id: id.to_string(),
            })?;

        Ok(EndpointHealthResponse {
            healthy: status.healthy,
            latency_ms: status.latency_ms,
            error: status.last_error,
            circuit_state: status.circuit_state,
            consecutive_failures: status.consecutive_failures,
            last_delivery_at: status.last_delivery_at,
            checked_at: status.last_check_at.unwrap_or(status.updated_at),
        })
    }

    // ============ 工作流管理 ============

    pub async fn list_workflows(&self) -> HsbResult<Vec<WorkflowResponse>> {
        if let Some(store) = &self.workflow_store {
            let items = store.list_workflows().await?;
            return Ok(items.iter().map(stored_workflow_to_response).collect());
        }

        let workflows = self.workflow_definitions.read().await;
        let mut items: Vec<_> = workflows.values().cloned().collect();
        items.sort_by(|left, right| left.workflow.name.cmp(&right.workflow.name));
        Ok(items.iter().map(workflow_record_to_response).collect())
    }

    pub async fn get_workflow(&self, id: &str) -> HsbResult<Option<WorkflowResponse>> {
        if let Some(store) = &self.workflow_store {
            return Ok(store
                .get_workflow(id)
                .await?
                .as_ref()
                .map(stored_workflow_to_response));
        }

        let workflows = self.workflow_definitions.read().await;
        Ok(workflows.get(id).map(workflow_record_to_response))
    }

    pub async fn create_workflow(&self, req: CreateWorkflowRequest) -> HsbResult<WorkflowResponse> {
        let workflow = build_workflow_from_create(req)?;

        if let Some(store) = &self.workflow_store {
            if store.get_workflow(&workflow.id).await?.is_some() {
                return Err(HsbError::DuplicateRecord {
                    entity: "Workflow".to_string(),
                    id: workflow.id.clone(),
                });
            }

            store.save_workflow(&workflow).await?;
            return self
                .get_workflow(&workflow.id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: workflow.id.clone(),
                });
        }

        let mut workflows = self.workflow_definitions.write().await;

        if workflows.contains_key(&workflow.id) {
            return Err(HsbError::DuplicateRecord {
                entity: "Workflow".to_string(),
                id: workflow.id.clone(),
            });
        }

        let created_at = Utc::now();
        let record = WorkflowDefinitionRecord {
            workflow,
            created_at,
            updated_at: created_at,
        };
        let response = workflow_record_to_response(&record);
        workflows.insert(record.workflow.id.clone(), record);
        Ok(response)
    }

    pub async fn update_workflow(
        &self,
        id: &str,
        req: UpdateWorkflowRequest,
    ) -> HsbResult<WorkflowResponse> {
        if let Some(store) = &self.workflow_store {
            let mut record = store
                .get_workflow(id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: id.to_string(),
                })?;

            apply_workflow_update(&mut record.workflow, req)?;
            store.save_workflow(&record.workflow).await?;
            return self
                .get_workflow(id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: id.to_string(),
                });
        }

        let mut workflows = self.workflow_definitions.write().await;
        let record = workflows.get_mut(id).ok_or_else(|| HsbError::NotFound {
            entity: "Workflow".to_string(),
            id: id.to_string(),
        })?;

        apply_workflow_update(&mut record.workflow, req)?;
        record.updated_at = Utc::now();

        Ok(workflow_record_to_response(record))
    }

    pub async fn delete_workflow(&self, id: &str) -> HsbResult<()> {
        if let Some(store) = &self.workflow_store {
            return store.delete_workflow(id).await;
        }

        let removed = self.workflow_definitions.write().await.remove(id);
        if removed.is_none() {
            return Err(HsbError::NotFound {
                entity: "Workflow".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    pub async fn list_workflow_instances(
        &self,
        params: WorkflowInstanceQueryParams,
    ) -> HsbResult<Vec<WorkflowInstanceResponse>> {
        let items = if let Some(executor) = &self.workflow_executor {
            executor.list_instances().await?
        } else if let Some(store) = &self.workflow_store {
            store
                .list_workflow_instances(&WorkflowInstanceQuery {
                    workflow_id: params.workflow_id.clone(),
                    status: params.status.clone(),
                    limit: Some(Self::bounded_limit(params.limit)),
                    offset: params.offset,
                })
                .await?
        } else {
            Vec::new()
        };

        let mut filtered = items;
        if let Some(workflow_id) = params.workflow_id {
            filtered.retain(|instance| instance.workflow_id == workflow_id);
        }
        if let Some(status) = params.status {
            filtered
                .retain(|instance| workflow_status_label(instance) == status.to_ascii_uppercase());
        }
        filtered.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let offset = params.offset.unwrap_or(0);
        let limit = Self::bounded_limit(params.limit);
        Ok(filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(workflow_instance_to_response)
            .collect())
    }

    pub async fn get_workflow_instance(
        &self,
        id: &str,
    ) -> HsbResult<Option<WorkflowInstanceResponse>> {
        if let Some(executor) = &self.workflow_executor {
            if let Some(instance) = executor.get_instance(parse_instance_id(id)?).await? {
                return Ok(Some(workflow_instance_to_response(instance)));
            }
        }

        if let Some(store) = &self.workflow_store {
            return Ok(store
                .get_workflow_instance(id)
                .await?
                .map(workflow_instance_to_response));
        }

        Ok(None)
    }

    pub async fn start_workflow_instance(
        &self,
        workflow_id: &str,
        req: StartWorkflowInstanceRequest,
    ) -> HsbResult<WorkflowInstanceResponse> {
        let workflow = self.load_workflow_definition(workflow_id).await?;
        let executor = self.require_workflow_executor()?;
        let input = build_workflow_input_message(req)?;
        let instance = executor.start(&workflow, input).await?;
        self.persist_workflow_instance(&instance).await?;
        Ok(workflow_instance_to_response(instance))
    }

    pub async fn resume_workflow_instance(&self, id: &str) -> HsbResult<WorkflowInstanceResponse> {
        let executor = self.require_workflow_executor()?;
        let instance = executor.resume(parse_instance_id(id)?).await?;
        self.persist_workflow_instance(&instance).await?;
        Ok(workflow_instance_to_response(instance))
    }

    pub async fn pause_workflow_instance(&self, id: &str) -> HsbResult<WorkflowInstanceResponse> {
        let executor = self.require_workflow_executor()?;
        let instance_id = parse_instance_id(id)?;
        executor.pause(instance_id).await?;
        let instance =
            executor
                .get_instance(instance_id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: id.to_string(),
                })?;
        self.persist_workflow_instance(&instance).await?;
        Ok(workflow_instance_to_response(instance))
    }

    pub async fn cancel_workflow_instance(&self, id: &str) -> HsbResult<WorkflowInstanceResponse> {
        let executor = self.require_workflow_executor()?;
        let instance_id = parse_instance_id(id)?;
        executor.cancel(instance_id).await?;
        let instance =
            executor
                .get_instance(instance_id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: id.to_string(),
                })?;
        self.persist_workflow_instance(&instance).await?;
        Ok(workflow_instance_to_response(instance))
    }

    pub async fn compensate_workflow_instance(
        &self,
        id: &str,
    ) -> HsbResult<WorkflowInstanceResponse> {
        let executor = self.require_workflow_executor()?;
        let instance_id = parse_instance_id(id)?;
        executor.compensate(instance_id).await?;
        let instance =
            executor
                .get_instance(instance_id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: id.to_string(),
                })?;
        self.persist_workflow_instance(&instance).await?;
        Ok(workflow_instance_to_response(instance))
    }

    // ============ 消息管理 ============

    pub async fn list_messages(
        &self,
        params: MessageQueryParams,
    ) -> HsbResult<Vec<MessageResponse>> {
        let store = self.require_message_store()?;
        let query = PersistentMessageQuery {
            source_system: params.source_system,
            target_system: params.target_system,
            message_type: params.message_type,
            status: params.status.map(|value| value.to_ascii_uppercase()),
            from_time: params.from_time,
            to_time: params.to_time,
            limit: Some(Self::bounded_limit(params.limit)),
            offset: params.offset,
        };

        let messages = store.list_messages(&query).await?;
        Ok(messages
            .into_iter()
            .map(|message| message_to_response(&message))
            .collect())
    }

    pub async fn get_message(&self, id: &str) -> HsbResult<Option<MessageResponse>> {
        let store = self.require_message_store()?;
        Ok(store
            .get_message(id)
            .await?
            .map(|message| message_to_response(&message)))
    }

    pub async fn reprocess_message(&self, id: &str) -> HsbResult<()> {
        let store = self.require_message_store()?;
        let replay = self.require_message_replay()?;
        let mut message = store
            .get_message(id)
            .await?
            .ok_or_else(|| HsbError::NotFound {
                entity: "Message".to_string(),
                id: id.to_string(),
            })?;
        message.increment_retry();
        replay.replay(message).await
    }

    // ============ 死信队列 ============

    pub async fn list_dlq(&self, params: DlqQueryParams) -> HsbResult<Vec<DlqMessageResponse>> {
        let filter = hsb_core::reliability::DeadLetterFilter {
            source_system: params.source_system,
            from_time: params.from_time,
            to_time: params.to_time,
            limit: Some(Self::bounded_limit(params.limit)),
            offset: params.offset,
            ..Default::default()
        };

        let letters = self.dlq.list(filter).await?;
        Ok(letters
            .into_iter()
            .map(|dl| dead_letter_to_response(&dl))
            .collect())
    }

    pub async fn dlq_stats(&self) -> HsbResult<DeadLetterStats> {
        self.dlq.stats().await
    }

    pub async fn get_dlq_message(&self, id: &str) -> HsbResult<Option<DlqMessageResponse>> {
        let letter = self.dlq.get(id).await?;
        Ok(letter.map(|dl| dead_letter_to_response(&dl)))
    }

    pub async fn reprocess_dlq_message(&self, id: &str) -> HsbResult<()> {
        let replay = self.require_message_replay()?;
        let mut msg = self.dlq.reprocess(id).await?;
        msg.increment_retry();
        replay.replay(msg).await
    }

    pub async fn delete_dlq_message(&self, id: &str) -> HsbResult<()> {
        self.dlq.delete(id).await
    }

    // ============ 审计 ============

    pub async fn query_audit(
        &self,
        params: AuditQueryParams,
    ) -> HsbResult<Vec<AuditEventResponse>> {
        let filter = AuditFilter {
            message_id: params.message_id,
            from_time: params.from_time,
            to_time: params.to_time,
            failed_only: params.failed_only.unwrap_or(false),
            limit: Some(Self::bounded_limit(params.limit)),
            offset: params.offset,
            ..Default::default()
        };

        let events = self.audit.query(filter).await?;
        Ok(events
            .into_iter()
            .map(|e| audit_event_to_response(&e))
            .collect())
    }

    pub async fn get_message_trace(&self, message_id: &str) -> HsbResult<MessageTrace> {
        self.audit.get_message_trace(message_id).await
    }

    // ============ 熔断器 ============

    pub async fn list_circuit_breakers(&self) -> Vec<CircuitBreakerResponse> {
        let stats = self.circuit_breakers.all_stats().await;
        stats
            .into_iter()
            .map(|s| CircuitBreakerResponse {
                name: s.name,
                state: format!("{:?}", s.state),
                failure_count: s.failure_count,
                success_count: s.success_count,
                last_failure: None,
            })
            .collect()
    }

    pub async fn reset_circuit_breaker(&self, name: &str) -> HsbResult<()> {
        let breaker = self.circuit_breakers.get_or_create(name).await;
        breaker.reset().await;
        Ok(())
    }

    // ============ 配置 ============

    pub async fn get_config(&self) -> serde_json::Value {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: serde_json::Value) -> HsbResult<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        Ok(())
    }

    pub async fn reload_config(&self) -> HsbResult<()> {
        // TODO: 从文件或远程重新加载配置
        Ok(())
    }

    async fn load_route(&self, id: &str) -> HsbResult<Route> {
        if let Some(store) = &self.route_store {
            return store
                .get_route(id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Route".to_string(),
                    id: id.to_string(),
                });
        }

        self.router
            .list_routes()
            .await?
            .into_iter()
            .find(|route| route.id.to_string() == id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "Route".to_string(),
                id: id.to_string(),
            })
    }

    fn require_endpoint_store(&self) -> HsbResult<&Arc<dyn EndpointStore>> {
        self.endpoint_store
            .as_ref()
            .ok_or_else(|| HsbError::ServiceUnavailable {
                service: "endpoint_store".to_string(),
            })
    }

    fn require_message_store(&self) -> HsbResult<&Arc<dyn PersistentMessageStore>> {
        self.message_store
            .as_ref()
            .ok_or_else(|| HsbError::ServiceUnavailable {
                service: "message_store".to_string(),
            })
    }

    fn require_message_replay(&self) -> HsbResult<&Arc<dyn MessageReplayService>> {
        self.message_replay
            .as_ref()
            .ok_or_else(|| HsbError::ServiceUnavailable {
                service: "message_replay".to_string(),
            })
    }

    fn require_workflow_executor(&self) -> HsbResult<&Arc<dyn WorkflowExecutor>> {
        self.workflow_executor
            .as_ref()
            .ok_or_else(|| HsbError::ServiceUnavailable {
                service: "workflow_executor".to_string(),
            })
    }

    async fn validate_custom_protocol_selection(
        &self,
        protocol: ProtocolType,
        properties: &HashMap<String, String>,
    ) -> HsbResult<()> {
        if protocol != ProtocolType::Custom {
            return Ok(());
        }

        let Some(custom_protocol_id) = properties
            .get("custom_protocol_id")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
        else {
            return Err(HsbError::InvalidField {
                field: "custom_protocol_id".to_string(),
                reason: "CUSTOM protocol requires selecting a concrete custom protocol definition"
                    .to_string(),
            });
        };

        match self.get_custom_protocol(custom_protocol_id).await? {
            Some(item) if item.enabled => Ok(()),
            Some(_) => Err(HsbError::InvalidField {
                field: "custom_protocol_id".to_string(),
                reason: format!("Custom protocol '{}' is disabled", custom_protocol_id),
            }),
            None => Err(HsbError::NotFound {
                entity: "CustomProtocol".to_string(),
                id: custom_protocol_id.to_string(),
            }),
        }
    }

    async fn load_workflow_definition(&self, id: &str) -> HsbResult<Workflow> {
        if let Some(store) = &self.workflow_store {
            return store
                .get_workflow(id)
                .await?
                .map(|value| value.workflow)
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: id.to_string(),
                });
        }

        self.workflow_definitions
            .read()
            .await
            .get(id)
            .map(|value| value.workflow.clone())
            .ok_or_else(|| HsbError::NotFound {
                entity: "Workflow".to_string(),
                id: id.to_string(),
            })
    }

    async fn persist_workflow_instance(&self, instance: &WorkflowInstance) -> HsbResult<()> {
        if let Some(store) = &self.workflow_store {
            store.save_workflow_instance(instance).await?;
        }
        Ok(())
    }

    async fn sync_endpoint_registry(
        &self,
        endpoint: &Endpoint,
        status: Option<&EndpointRuntimeStatus>,
    ) {
        let mut endpoints = self.endpoints.write().await;
        let mut info = EndpointInfo::new(
            endpoint.id.as_str(),
            &endpoint.name,
            endpoint.protocol,
            &endpoint.address(),
        );
        info.enabled = endpoint.enabled;
        info.healthy = status
            .map(|value| value.healthy)
            .unwrap_or(endpoint.enabled);
        info.last_heartbeat = status.and_then(|value| value.last_check_at);
        endpoints.register(info);
    }

    async fn load_or_initialize_endpoint_status(
        &self,
        endpoint: &Endpoint,
    ) -> HsbResult<EndpointRuntimeStatus> {
        if let Some(store) = &self.endpoint_store {
            if let Some(status) = store.get_endpoint_status(endpoint.id.as_str()).await? {
                return Ok(status);
            }

            let status = default_endpoint_runtime_status(endpoint);
            store.upsert_endpoint_status(&status).await?;
            return Ok(status);
        }

        Ok(default_endpoint_runtime_status(endpoint))
    }

    async fn load_organization(&self, id: &str) -> HsbResult<Organization> {
        if let Some(store) = &self.organization_store {
            return store
                .get_organization(id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Organization".to_string(),
                    id: id.to_string(),
                });
        }

        self.organizations
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| HsbError::NotFound {
                entity: "Organization".to_string(),
                id: id.to_string(),
            })
    }

    async fn load_system(&self, id: &str) -> HsbResult<IntegrationSystem> {
        if let Some(store) = &self.system_store {
            return store
                .get_system(id)
                .await?
                .ok_or_else(|| HsbError::NotFound {
                    entity: "IntegrationSystem".to_string(),
                    id: id.to_string(),
                });
        }

        self.systems
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| HsbError::NotFound {
                entity: "IntegrationSystem".to_string(),
                id: id.to_string(),
            })
    }
}

// ============ 转换辅助函数 ============

fn route_to_response(route: &Route) -> RouteResponse {
    RouteResponse {
        id: route.id.to_string(),
        name: route.name.clone(),
        description: route.description.clone(),
        source_system: route.source_match.system_id.clone(),
        message_type: route.source_match.message_type_pattern.clone(),
        protocol: route.source_match.protocol,
        conditions: route.conditions.clone(),
        targets: route
            .targets
            .iter()
            .map(|t| RouteTargetResponse {
                system_id: t.endpoint_id.to_string(),
                endpoint: t.endpoint_id.to_string(),
                transport: if t.primary {
                    "primary".to_string()
                } else if t.fallback {
                    "fallback".to_string()
                } else {
                    "weighted".to_string()
                },
                timeout_secs: route.options.timeout_ms / 1000,
            })
            .collect(),
        transformer_ids: route.transformer_ids.clone(),
        priority: route.priority,
        enabled: route.enabled,
        delivery_mode: route.options.delivery_mode,
        timeout_ms: route.options.timeout_ms,
        async_delivery: route.options.async_delivery,
        require_ack: route.options.require_ack,
        audit_enabled: route.options.audit_enabled,
        dlq_on_failure: route.options.dlq_on_failure,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn build_route_from_create(req: CreateRouteRequest) -> HsbResult<Route> {
    if req.targets.is_empty() {
        return Err(HsbError::ValidationError {
            message: "At least one route target is required".to_string(),
        });
    }

    Ok(Route {
        id: hsb_common::RouteId::new(req.id.unwrap_or_else(|| ulid::Ulid::new().to_string())),
        name: req.name,
        description: req.description,
        source_match: SourceMatch {
            system_id: req.source_system,
            protocol: req.protocol,
            message_type_pattern: req.message_type,
        },
        conditions: req.conditions.unwrap_or_default(),
        targets: build_route_targets(req.targets)?,
        transformer_ids: req.transformer_ids.unwrap_or_default(),
        priority: req.priority.unwrap_or(0),
        enabled: req.enabled.unwrap_or(true),
        options: RouteOptions {
            delivery_mode: req.delivery_mode.unwrap_or(DeliveryMode::AtLeastOnce),
            timeout_ms: req.timeout_ms.unwrap_or(30_000),
            async_delivery: req.async_delivery.unwrap_or(false),
            require_ack: req.require_ack.unwrap_or(true),
            audit_enabled: req.audit_enabled.unwrap_or(true),
            dlq_on_failure: req.dlq_on_failure.unwrap_or(true),
        },
    })
}

fn build_route_targets(targets: Vec<RouteTargetRequest>) -> HsbResult<Vec<RouteTarget>> {
    if targets.is_empty() {
        return Err(HsbError::ValidationError {
            message: "At least one route target is required".to_string(),
        });
    }

    Ok(targets
        .into_iter()
        .enumerate()
        .map(|(index, target)| RouteTarget {
            endpoint_id: SystemId::new(target.system_id),
            weight: 100,
            primary: index == 0,
            fallback: index > 0,
            transformer_ids: Vec::new(),
        })
        .collect())
}

fn merge_route_options(route_options: &mut RouteOptions, req: &UpdateRouteRequest) {
    if let Some(delivery_mode) = req.delivery_mode {
        route_options.delivery_mode = delivery_mode;
    }
    if let Some(timeout_ms) = req.timeout_ms {
        route_options.timeout_ms = timeout_ms;
    }
    if let Some(async_delivery) = req.async_delivery {
        route_options.async_delivery = async_delivery;
    }
    if let Some(require_ack) = req.require_ack {
        route_options.require_ack = require_ack;
    }
    if let Some(audit_enabled) = req.audit_enabled {
        route_options.audit_enabled = audit_enabled;
    }
    if let Some(dlq_on_failure) = req.dlq_on_failure {
        route_options.dlq_on_failure = dlq_on_failure;
    }
}

fn apply_workflow_update(workflow: &mut Workflow, req: UpdateWorkflowRequest) -> HsbResult<()> {
    if let Some(name) = req.name {
        workflow.name = name;
    }
    if let Some(description) = req.description {
        workflow.description = Some(description);
    }
    if let Some(enabled) = req.enabled {
        workflow.enabled = enabled;
    }
    if let Some(timeout_ms) = req.timeout_ms {
        workflow.timeout = Duration::from_millis(timeout_ms);
    }
    if let Some(options) = req.options {
        workflow.options = options;
    }
    if req.clear_compensation.unwrap_or(false) {
        workflow.compensation = None;
    }
    if let Some(compensation) = req.compensation {
        workflow.compensation = Some(compensation_policy_from_payload(compensation));
    }
    if let Some(steps) = req.steps {
        if steps.is_empty() {
            return Err(HsbError::ValidationError {
                message: "Workflow must contain at least one step".to_string(),
            });
        }
        workflow.steps = steps
            .into_iter()
            .map(workflow_step_from_payload)
            .collect::<HsbResult<Vec<_>>>()?;
    }

    workflow.version = req
        .version
        .unwrap_or_else(|| workflow.version.saturating_add(1));
    Ok(())
}

fn build_workflow_from_create(req: CreateWorkflowRequest) -> HsbResult<Workflow> {
    if req.steps.is_empty() {
        return Err(HsbError::ValidationError {
            message: "Workflow must contain at least one step".to_string(),
        });
    }

    Ok(Workflow {
        id: req.id.unwrap_or_else(|| ulid::Ulid::new().to_string()),
        name: req.name,
        description: req.description,
        version: req.version.unwrap_or(1),
        steps: req
            .steps
            .into_iter()
            .map(workflow_step_from_payload)
            .collect::<HsbResult<Vec<_>>>()?,
        timeout: Duration::from_millis(req.timeout_ms.unwrap_or(3_600_000)),
        compensation: req.compensation.map(compensation_policy_from_payload),
        enabled: req.enabled.unwrap_or(true),
        options: req.options.unwrap_or_else(WorkflowOptions::default),
    })
}

fn build_workflow_input_message(req: StartWorkflowInstanceRequest) -> HsbResult<Message> {
    let raw_payload = req
        .raw_payload_text
        .map(|value| value.into_bytes())
        .unwrap_or_else(|| {
            req.payload
                .as_ref()
                .map(|value| serde_json::to_vec(value).unwrap_or_default())
                .unwrap_or_default()
        });

    let mut builder = MessageBuilder::new()
        .source_system(req.source_system)
        .protocol(req.protocol)
        .raw_payload(raw_payload);

    if let Some(target_system) = req.target_system {
        builder = builder.target_system(target_system);
    }
    if let Some(message_type) = req.message_type {
        builder = builder.message_type(message_type);
    }
    if let Some(correlation_id) = req.correlation_id {
        builder = builder.correlation_id(correlation_id);
    }
    if let Some(payload) = req.payload {
        builder = builder.payload(payload);
    }

    builder.build()
}

fn workflow_step_from_payload(step: WorkflowStepPayload) -> HsbResult<WorkflowStep> {
    if step.id.trim().is_empty() {
        return Err(HsbError::ValidationError {
            message: "Workflow step id cannot be empty".to_string(),
        });
    }

    Ok(WorkflowStep {
        id: step.id,
        name: step.name,
        step_type: step.step_type,
        config: step.config.unwrap_or_default(),
        retry: step.retry,
        timeout: step.timeout_ms.map(Duration::from_millis),
        condition: step.condition,
        compensation_step: step
            .compensation_step
            .map(|value| workflow_step_from_payload(*value).map(Box::new))
            .transpose()?,
        next_steps: step.next_steps.unwrap_or_default(),
    })
}

fn workflow_record_to_response(record: &WorkflowDefinitionRecord) -> WorkflowResponse {
    WorkflowResponse {
        id: record.workflow.id.clone(),
        name: record.workflow.name.clone(),
        description: record.workflow.description.clone(),
        version: record.workflow.version,
        enabled: record.workflow.enabled,
        timeout_ms: record.workflow.timeout.as_millis() as u64,
        compensation: record
            .workflow
            .compensation
            .as_ref()
            .map(compensation_policy_to_payload),
        options: record.workflow.options.clone(),
        steps: record
            .workflow
            .steps
            .iter()
            .map(workflow_step_to_payload)
            .collect(),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn stored_workflow_to_response(record: &StoredWorkflowDefinition) -> WorkflowResponse {
    WorkflowResponse {
        id: record.workflow.id.clone(),
        name: record.workflow.name.clone(),
        description: record.workflow.description.clone(),
        version: record.workflow.version,
        enabled: record.workflow.enabled,
        timeout_ms: record.workflow.timeout.as_millis() as u64,
        compensation: record
            .workflow
            .compensation
            .as_ref()
            .map(compensation_policy_to_payload),
        options: record.workflow.options.clone(),
        steps: record
            .workflow
            .steps
            .iter()
            .map(workflow_step_to_payload)
            .collect(),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn workflow_instance_to_response(instance: WorkflowInstance) -> WorkflowInstanceResponse {
    let status = workflow_status_label(&instance);
    WorkflowInstanceResponse {
        id: instance.id.to_string(),
        workflow_id: instance.workflow_id,
        workflow_version: instance.workflow_version,
        status,
        current_step_id: instance.current_step_id,
        context: instance.context,
        step_history: instance.step_history,
        created_at: instance.created_at,
        updated_at: instance.updated_at,
        completed_at: instance.completed_at,
        error: instance.error,
    }
}

fn validate_custom_protocol_fields(fields: &[CustomProtocolFieldDefinition]) -> HsbResult<()> {
    if fields.is_empty() {
        return Err(HsbError::InvalidField {
            field: "fields".to_string(),
            reason: "Custom protocol requires at least one field definition".to_string(),
        });
    }

    let mut names = std::collections::HashSet::new();
    for field in fields {
        if field.name.trim().is_empty() {
            return Err(HsbError::InvalidField {
                field: "fields.name".to_string(),
                reason: "Field name cannot be empty".to_string(),
            });
        }
        if !names.insert(field.name.clone()) {
            return Err(HsbError::DuplicateRecord {
                entity: "CustomProtocolField".to_string(),
                id: field.name.clone(),
            });
        }
    }

    Ok(())
}

fn validate_endpoint_protocol_properties(
    protocol: ProtocolType,
    properties: &HashMap<String, String>,
) -> HsbResult<()> {
    match protocol {
        ProtocolType::Database => {
            let Some(database_type) = properties
                .get("database_type")
                .map(|value| value.trim().to_ascii_lowercase())
                .filter(|value| !value.is_empty())
            else {
                return Err(HsbError::InvalidField {
                    field: "database_type".to_string(),
                    reason: "DATABASE endpoint requires selecting a database type".to_string(),
                });
            };

            let supported = [
                "postgresql",
                "oracle",
                "mysql",
                "sqlserver",
                "hive",
                "clickhouse",
            ];
            if !supported.contains(&database_type.as_str()) {
                return Err(HsbError::InvalidField {
                    field: "database_type".to_string(),
                    reason: format!(
                        "Unsupported database type '{}'. Supported: {}",
                        database_type,
                        supported.join(", ")
                    ),
                });
            }
            Ok(())
        }
        ProtocolType::OpenAi => {
            let has_model = properties
                .get("model")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false);
            if !has_model {
                return Err(HsbError::InvalidField {
                    field: "model".to_string(),
                    reason: "OPENAI endpoint requires a default model".to_string(),
                });
            }
            Ok(())
        }
        ProtocolType::Webhook => {
            let method = properties
                .get("webhook_method")
                .map(|value| value.trim().to_ascii_uppercase())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "POST".to_string());
            let supported = ["POST"];
            if !supported.contains(&method.as_str()) {
                return Err(HsbError::InvalidField {
                    field: "webhook_method".to_string(),
                    reason: format!(
                        "Unsupported webhook method '{}'. Supported: {}",
                        method,
                        supported.join(", ")
                    ),
                });
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn parse_topic_response(
    topic_name: String,
    description: Option<String>,
    owner_system_id: Option<String>,
    enabled: bool,
    properties: HashMap<String, String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> HsbResult<TopicResponse> {
    let topic = Topic::parse(topic_name).map_err(|reason| HsbError::InvalidField {
        field: "topic".to_string(),
        reason,
    })?;

    Ok(TopicResponse {
        id: topic.as_str().to_string(),
        topic: topic.as_str().to_string(),
        domain: topic.domain().to_string(),
        service: topic.service().to_string(),
        action: topic.action().to_string(),
        version: topic.version().to_string(),
        description,
        owner_system_id,
        enabled,
        properties,
        created_at,
        updated_at,
    })
}

fn topic_response_to_stored(topic: &TopicResponse) -> StoredTopic {
    StoredTopic {
        topic: topic.topic.clone(),
        description: topic.description.clone(),
        owner_system_id: topic.owner_system_id.clone(),
        enabled: topic.enabled,
        properties: topic.properties.clone(),
        created_at: topic.created_at,
        updated_at: topic.updated_at,
    }
}

fn stored_topic_to_response(topic: StoredTopic) -> HsbResult<TopicResponse> {
    parse_topic_response(
        topic.topic,
        topic.description,
        topic.owner_system_id,
        topic.enabled,
        topic.properties,
        topic.created_at,
        topic.updated_at,
    )
}

fn custom_protocol_response_to_stored(
    protocol: &CustomProtocolResponse,
) -> HsbResult<StoredCustomProtocol> {
    Ok(StoredCustomProtocol {
        id: protocol.id.clone(),
        name: protocol.name.clone(),
        description: protocol.description.clone(),
        transport_hint: protocol.transport_hint.clone(),
        content_type: protocol.content_type.clone(),
        fields: serde_json::to_value(&protocol.fields).map_err(|e| {
            HsbError::SerializationError {
                message: format!("Failed to serialize custom protocol fields: {}", e),
            }
        })?,
        sample_payload: protocol.sample_payload.clone(),
        enabled: protocol.enabled,
        created_at: protocol.created_at,
        updated_at: protocol.updated_at,
    })
}

fn stored_custom_protocol_to_response(
    protocol: StoredCustomProtocol,
) -> HsbResult<CustomProtocolResponse> {
    Ok(CustomProtocolResponse {
        id: protocol.id,
        name: protocol.name,
        description: protocol.description,
        transport_hint: protocol.transport_hint,
        content_type: protocol.content_type,
        fields: serde_json::from_value(protocol.fields).map_err(|e| {
            HsbError::SerializationError {
                message: format!("Failed to parse custom protocol fields: {}", e),
            }
        })?,
        sample_payload: protocol.sample_payload,
        enabled: protocol.enabled,
        created_at: protocol.created_at,
        updated_at: protocol.updated_at,
    })
}

fn workflow_status_label(instance: &WorkflowInstance) -> String {
    format!("{:?}", instance.status).to_ascii_uppercase()
}

fn parse_instance_id(id: &str) -> HsbResult<ulid::Ulid> {
    id.parse::<ulid::Ulid>()
        .map_err(|e| HsbError::InvalidField {
            field: "workflow_instance_id".to_string(),
            reason: e.to_string(),
        })
}

fn workflow_step_to_payload(step: &WorkflowStep) -> WorkflowStepPayload {
    WorkflowStepPayload {
        id: step.id.clone(),
        name: step.name.clone(),
        step_type: step.step_type.clone(),
        config: Some(step.config.clone()),
        retry: step.retry.clone(),
        timeout_ms: step.timeout.map(|value| value.as_millis() as u64),
        condition: step.condition.clone(),
        compensation_step: step
            .compensation_step
            .as_ref()
            .map(|value| Box::new(workflow_step_to_payload(value))),
        next_steps: Some(step.next_steps.clone()),
    }
}

fn compensation_policy_from_payload(
    payload: WorkflowCompensationPolicyPayload,
) -> CompensationPolicy {
    CompensationPolicy {
        mode: payload.mode,
        timeout: Duration::from_millis(payload.timeout_ms),
        continue_on_failure: payload.continue_on_failure,
    }
}

fn compensation_policy_to_payload(
    policy: &CompensationPolicy,
) -> WorkflowCompensationPolicyPayload {
    WorkflowCompensationPolicyPayload {
        mode: policy.mode,
        timeout_ms: policy.timeout.as_millis() as u64,
        continue_on_failure: policy.continue_on_failure,
    }
}

fn endpoint_info_to_response(endpoint: &EndpointInfo) -> EndpointResponse {
    EndpointResponse {
        id: endpoint.id.clone(),
        organization_id: None,
        system_id: None,
        name: endpoint.name.clone(),
        description: None,
        system_type: hsb_common::MedicalSystemType::Other,
        protocol: endpoint.protocol,
        roles: vec![],
        connection: endpoint_info_to_connection(endpoint),
        auth: None,
        config: Default::default(),
        enabled: endpoint.enabled,
        lifecycle_status: if endpoint.enabled {
            EndpointLifecycleStatus::Active
        } else {
            EndpointLifecycleStatus::Disabled
        },
        version: 1,
        security: EndpointSecurity::default(),
        properties: HashMap::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        created_by: None,
        updated_by: None,
        status: Some(EndpointStatusResponse {
            healthy: endpoint.healthy,
            latency_ms: None,
            last_error: None,
            circuit_state: None,
            consecutive_failures: 0,
            last_check_at: endpoint.last_heartbeat,
            last_delivery_at: None,
            updated_at: endpoint.last_heartbeat.unwrap_or_else(chrono::Utc::now),
        }),
    }
}

fn endpoint_to_response(
    endpoint: &Endpoint,
    status: Option<&EndpointRuntimeStatus>,
) -> EndpointResponse {
    EndpointResponse {
        id: endpoint.id.to_string(),
        organization_id: Some(endpoint.organization_id.to_string()),
        system_id: Some(endpoint.system_id.to_string()),
        name: endpoint.name.clone(),
        description: endpoint.description.clone(),
        system_type: endpoint.system_type,
        protocol: endpoint.protocol,
        roles: endpoint.roles.clone(),
        connection: endpoint.connection.clone(),
        auth: endpoint.auth.as_ref().map(auth_to_summary),
        config: endpoint.config.clone(),
        enabled: endpoint.enabled,
        lifecycle_status: endpoint.lifecycle_status,
        version: endpoint.version,
        security: endpoint.security.clone(),
        properties: endpoint.properties.clone(),
        created_at: endpoint.created_at,
        updated_at: endpoint.updated_at,
        created_by: endpoint.created_by.clone(),
        updated_by: endpoint.updated_by.clone(),
        status: status.map(endpoint_status_to_response),
    }
}

fn endpoint_status_to_response(status: &EndpointRuntimeStatus) -> EndpointStatusResponse {
    EndpointStatusResponse {
        healthy: status.healthy,
        latency_ms: status.latency_ms,
        last_error: status.last_error.clone(),
        circuit_state: status.circuit_state.clone(),
        consecutive_failures: status.consecutive_failures,
        last_check_at: status.last_check_at,
        last_delivery_at: status.last_delivery_at,
        updated_at: status.updated_at,
    }
}

fn default_endpoint_runtime_status(endpoint: &Endpoint) -> EndpointRuntimeStatus {
    let now = chrono::Utc::now();
    let mut status = EndpointRuntimeStatus::new(endpoint.id.as_str().to_string());
    status.healthy = endpoint.enabled;
    status.last_check_at = Some(now);
    status.updated_at = now;
    status
}

fn endpoint_info_to_connection(endpoint: &EndpointInfo) -> hsb_core::ConnectionConfig {
    if let Some((scheme, remainder)) = endpoint.address.split_once("://") {
        let tls_enabled = matches!(scheme, "https" | "grpcs");
        let (host_port, path) = match remainder.split_once('/') {
            Some((host_port, path)) => (host_port, Some(format!("/{}", path))),
            None => (remainder, None),
        };
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) => (host.to_string(), port.parse::<u16>().unwrap_or(80)),
            None => (
                host_port.to_string(),
                endpoint.protocol.default_port().unwrap_or(80),
            ),
        };

        let mut connection = hsb_core::ConnectionConfig::http(host, port);
        connection.path = path;
        connection.tls_enabled = tls_enabled;
        return connection;
    }

    let (host, port) = endpoint
        .address
        .rsplit_once(':')
        .map(|(host, port)| (host.to_string(), port.parse::<u16>().unwrap_or(80)))
        .unwrap_or_else(|| {
            (
                endpoint.address.clone(),
                endpoint.protocol.default_port().unwrap_or(80),
            )
        });
    hsb_core::ConnectionConfig::tcp(host, port)
}

fn auth_to_summary(auth: &AuthConfig) -> EndpointAuthSummaryResponse {
    match auth {
        AuthConfig::None => EndpointAuthSummaryResponse {
            auth_type: "none".to_string(),
            principal: None,
            header_name: None,
            token_url: None,
            scope: None,
            external_reference: None,
            secret_configured: false,
        },
        AuthConfig::Basic { username, .. } => EndpointAuthSummaryResponse {
            auth_type: "basic".to_string(),
            principal: Some(username.clone()),
            header_name: None,
            token_url: None,
            scope: None,
            external_reference: None,
            secret_configured: true,
        },
        AuthConfig::Bearer { .. } => EndpointAuthSummaryResponse {
            auth_type: "bearer".to_string(),
            principal: None,
            header_name: None,
            token_url: None,
            scope: None,
            external_reference: None,
            secret_configured: true,
        },
        AuthConfig::ApiKey { header_name, .. } => EndpointAuthSummaryResponse {
            auth_type: "api_key".to_string(),
            principal: None,
            header_name: Some(header_name.clone()),
            token_url: None,
            scope: None,
            external_reference: None,
            secret_configured: true,
        },
        AuthConfig::OAuth2ClientCredentials {
            client_id,
            token_url,
            scope,
            ..
        } => EndpointAuthSummaryResponse {
            auth_type: "oauth2_client_credentials".to_string(),
            principal: Some(client_id.clone()),
            header_name: None,
            token_url: Some(token_url.clone()),
            scope: scope.clone(),
            external_reference: None,
            secret_configured: true,
        },
        AuthConfig::Certificate { cert_path, .. } => EndpointAuthSummaryResponse {
            auth_type: "certificate".to_string(),
            principal: None,
            header_name: None,
            token_url: None,
            scope: None,
            external_reference: Some(cert_path.clone()),
            secret_configured: true,
        },
        AuthConfig::SsoToken {
            sso_endpoint,
            app_id,
        } => EndpointAuthSummaryResponse {
            auth_type: "sso_token".to_string(),
            principal: Some(app_id.clone()),
            header_name: None,
            token_url: None,
            scope: None,
            external_reference: Some(sso_endpoint.clone()),
            secret_configured: true,
        },
    }
}

fn merge_security(security: &mut EndpointSecurity, req: &UpdateEndpointSecurityRequest) {
    if let Some(secret_ref) = &req.secret_ref {
        security.secret_ref = Some(secret_ref.clone());
    }
    if let Some(require_tls) = req.require_tls {
        security.require_tls = require_tls;
    }
    if let Some(encryption_algorithm) = req.encryption_algorithm {
        security.encryption_algorithm = encryption_algorithm;
    }
    if let Some(allow_insecure_skip_verify) = req.allow_insecure_skip_verify {
        security.allow_insecure_skip_verify = allow_insecure_skip_verify;
    }
    if let Some(allowed_ip_ranges) = &req.allowed_ip_ranges {
        security.allowed_ip_ranges = allowed_ip_ranges.clone();
    }
    if let Some(mask_credentials_in_logs) = req.mask_credentials_in_logs {
        security.mask_credentials_in_logs = mask_credentials_in_logs;
    }
    if let Some(credential_expires_at) = req.credential_expires_at {
        security.credential_expires_at = Some(credential_expires_at);
    }
    security.credential_last_rotated_at = Some(chrono::Utc::now());
}

fn dead_letter_to_response(dl: &DeadLetter) -> DlqMessageResponse {
    DlqMessageResponse {
        id: dl.message.id.to_string(),
        message_id: dl.message.id.to_string(),
        reason: dl.reason.as_str().to_string(),
        error_detail: dl.error_detail.clone(),
        retry_count: dl.retry_count,
        source_system: dl.message.source_system.to_string(),
        target_system: dl.target_system.clone(),
        dead_lettered_at: dl.dead_lettered_at,
    }
}

fn message_to_response(message: &Message) -> MessageResponse {
    MessageResponse {
        id: message.id.to_string(),
        source_system: message.source_system.to_string(),
        target_system: message
            .target_system
            .as_ref()
            .map(|value| value.to_string()),
        protocol: message.protocol.to_string(),
        message_type: message.message_type.clone(),
        status: format!("{:?}", message.status),
        created_at: message.created_at,
        processed_at: message.status.is_terminal().then_some(message.updated_at),
    }
}

fn audit_event_to_response(event: &AuditEvent) -> AuditEventResponse {
    AuditEventResponse {
        id: event.id.clone(),
        trace_id: event.trace_id.as_ref().map(|t| t.to_string()),
        message_id: event.message_id.clone(),
        event_type: event.event_type.as_str().to_string(),
        severity: format!("{:?}", event.severity),
        timestamp: event.timestamp,
        source_system: event.source_system.clone(),
        target_system: event.target_system.clone(),
        component: event.component.clone(),
        description: event.description.clone(),
        success: event.success,
        error: event.error.clone(),
        duration_ms: event.duration_ms,
    }
}
