//! PostgreSQL 持久化存储
//!
//! 用于：
//! - 消息全量归档
//! - 路由规则持久化
//! - 审计合规记录
//! - 死信队列持久化

use async_trait::async_trait;
use std::time::Duration;

use hsb_common::{
    HsbError, HsbResult, MessagePriority, MessageStatus, OrganizationId, OrganizationType,
    ProtocolType, SystemId, Topic, TraceId,
};
use tracing::info;

use crate::workflow::WorkflowInstance;
use crate::{
    Endpoint, EndpointRuntimeStatus, EndpointVersionRecord, IntegrationSystem, Message,
    Organization, Workflow,
};
use serde_json::Value;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, QueryBuilder, Row};

use super::{
    EndpointStore, IntegrationSystemStore, OrganizationStore, PersistentMessageQuery,
    PersistentMessageStore, PostgresConfig, RouteStore, StoredWorkflowDefinition,
    WorkflowInstanceQuery, WorkflowStore,
};

/// PostgreSQL 持久化存储
#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    /// 连接 PostgreSQL 并初始化表结构
    pub async fn connect(config: &PostgresConfig) -> HsbResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_secs))
            .connect(&config.url)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to connect to PostgreSQL: {}", e),
            })?;

        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    /// 运行数据库迁移
    async fn run_migrations(&self) -> HsbResult<()> {
        for ddl in &[
            r#"
            CREATE TABLE IF NOT EXISTS hsb_messages (
                id          TEXT PRIMARY KEY,
                version     INTEGER NOT NULL DEFAULT 1,
                source_system TEXT NOT NULL,
                target_system TEXT,
                protocol    TEXT NOT NULL,
                message_type TEXT,
                headers     JSONB NOT NULL DEFAULT '{}',
                payload     JSONB,
                raw_payload BYTEA NOT NULL,
                status      TEXT NOT NULL,
                priority    TEXT NOT NULL DEFAULT 'NORMAL',
                trace_id    TEXT NOT NULL,
                correlation_id TEXT,
                metadata    JSONB NOT NULL DEFAULT '{}',
                created_at  TIMESTAMPTZ NOT NULL,
                updated_at  TIMESTAMPTZ NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_routes (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                config      JSONB NOT NULL,
                enabled     BOOLEAN NOT NULL DEFAULT true,
                priority    INTEGER NOT NULL DEFAULT 0,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_dead_letters (
                id              TEXT PRIMARY KEY,
                message_id      TEXT NOT NULL,
                reason          TEXT NOT NULL,
                error_detail    TEXT,
                retry_count     INTEGER NOT NULL DEFAULT 0,
                source_route_id TEXT,
                message_data    JSONB NOT NULL,
                created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_audit_log (
                id          TEXT PRIMARY KEY,
                trace_id    TEXT,
                message_id  TEXT,
                event_type  TEXT NOT NULL,
                severity    TEXT NOT NULL,
                timestamp   TIMESTAMPTZ NOT NULL,
                component   TEXT NOT NULL,
                description TEXT NOT NULL,
                success     BOOLEAN NOT NULL,
                error       TEXT,
                duration_ms BIGINT,
                metadata    JSONB NOT NULL DEFAULT '{}',
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            r#"
            CREATE UNLOGGED TABLE IF NOT EXISTS hsb_cache (
                key         TEXT PRIMARY KEY,
                value       JSONB NOT NULL,
                expires_at  TIMESTAMPTZ,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_organizations (
                id                  TEXT PRIMARY KEY,
                name                TEXT NOT NULL,
                description         TEXT,
                organization_type   TEXT NOT NULL,
                parent_organization_id TEXT,
                enabled             BOOLEAN NOT NULL DEFAULT true,
                properties          JSONB NOT NULL DEFAULT '{}',
                created_at          TIMESTAMPTZ NOT NULL,
                updated_at          TIMESTAMPTZ NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_integration_systems (
                id              TEXT PRIMARY KEY,
                organization_id TEXT NOT NULL,
                name            TEXT NOT NULL,
                description     TEXT,
                system_type     TEXT NOT NULL,
                topic_namespace TEXT,
                topic_prefix    TEXT,
                enabled         BOOLEAN NOT NULL DEFAULT true,
                properties      JSONB NOT NULL DEFAULT '{}',
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL,
                CONSTRAINT fk_system_org FOREIGN KEY (organization_id) REFERENCES hsb_organizations(id)
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_endpoints (
                id              TEXT PRIMARY KEY,
                organization_id TEXT,
                system_id       TEXT,
                name            TEXT NOT NULL,
                description     TEXT,
                system_type     TEXT NOT NULL,
                protocol        TEXT NOT NULL,
                roles           JSONB NOT NULL DEFAULT '[]',
                connection      JSONB NOT NULL,
                auth            JSONB,
                config          JSONB NOT NULL,
                enabled         BOOLEAN NOT NULL DEFAULT true,
                lifecycle_status TEXT NOT NULL,
                version         INTEGER NOT NULL DEFAULT 1,
                security        JSONB NOT NULL DEFAULT '{}',
                properties      JSONB NOT NULL DEFAULT '{}',
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL,
                created_by      TEXT,
                updated_by      TEXT
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_endpoint_versions (
                endpoint_id     TEXT NOT NULL,
                version         INTEGER NOT NULL,
                snapshot        JSONB NOT NULL,
                changed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                changed_by      TEXT,
                change_note     TEXT,
                PRIMARY KEY (endpoint_id, version)
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_endpoint_status (
                endpoint_id           TEXT PRIMARY KEY,
                healthy               BOOLEAN NOT NULL DEFAULT false,
                latency_ms            BIGINT,
                last_error            TEXT,
                circuit_state         TEXT,
                consecutive_failures  INTEGER NOT NULL DEFAULT 0,
                last_check_at         TIMESTAMPTZ,
                last_delivery_at      TIMESTAMPTZ,
                updated_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_endpoint_status_endpoint
                    FOREIGN KEY (endpoint_id) REFERENCES hsb_endpoints(id) ON DELETE CASCADE
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_workflows (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                description TEXT,
                version     INTEGER NOT NULL DEFAULT 1,
                enabled     BOOLEAN NOT NULL DEFAULT true,
                workflow    JSONB NOT NULL,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS hsb_workflow_instances (
                id              TEXT PRIMARY KEY,
                workflow_id     TEXT NOT NULL,
                workflow_version INTEGER NOT NULL,
                status          TEXT NOT NULL,
                current_step_id TEXT,
                instance_data   JSONB NOT NULL,
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL,
                completed_at    TIMESTAMPTZ,
                CONSTRAINT fk_workflow_instance_workflow
                    FOREIGN KEY (workflow_id) REFERENCES hsb_workflows(id) ON DELETE CASCADE
            )
            "#,
        ] {
            sqlx::query(ddl)
                .execute(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Migration failed: {}", e),
                })?;
        }

        for ddl in &[
            "ALTER TABLE hsb_messages ADD COLUMN IF NOT EXISTS topic TEXT",
            "ALTER TABLE hsb_messages ADD COLUMN IF NOT EXISTS meta JSONB NOT NULL DEFAULT '{}'",
            "ALTER TABLE hsb_endpoints ADD COLUMN IF NOT EXISTS organization_id TEXT",
            "ALTER TABLE hsb_endpoints ADD COLUMN IF NOT EXISTS system_id TEXT",
            "ALTER TABLE hsb_endpoints ADD COLUMN IF NOT EXISTS roles JSONB NOT NULL DEFAULT '[]'",
        ] {
            sqlx::query(ddl)
                .execute(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Migration alter failed: {}", e),
                })?;
        }

        for idx_sql in &[
            "CREATE INDEX IF NOT EXISTS idx_messages_status ON hsb_messages(status)",
            "CREATE INDEX IF NOT EXISTS idx_messages_created ON hsb_messages(created_at)",
            "CREATE INDEX IF NOT EXISTS idx_messages_source ON hsb_messages(source_system)",
            "CREATE INDEX IF NOT EXISTS idx_messages_trace ON hsb_messages(trace_id)",
            "CREATE INDEX IF NOT EXISTS idx_dead_letters_reason ON hsb_dead_letters(reason)",
            "CREATE INDEX IF NOT EXISTS idx_dead_letters_created ON hsb_dead_letters(created_at)",
            "CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON hsb_audit_log(timestamp)",
            "CREATE INDEX IF NOT EXISTS idx_audit_event_type ON hsb_audit_log(event_type)",
            "CREATE INDEX IF NOT EXISTS idx_audit_trace ON hsb_audit_log(trace_id)",
            "CREATE INDEX IF NOT EXISTS idx_cache_expires ON hsb_cache(expires_at) WHERE expires_at IS NOT NULL",
            "CREATE INDEX IF NOT EXISTS idx_org_type ON hsb_organizations(organization_type)",
            "CREATE INDEX IF NOT EXISTS idx_system_org ON hsb_integration_systems(organization_id)",
            "CREATE INDEX IF NOT EXISTS idx_system_type ON hsb_integration_systems(system_type)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_protocol ON hsb_endpoints(protocol)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_system_type ON hsb_endpoints(system_type)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_org ON hsb_endpoints(organization_id)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_system ON hsb_endpoints(system_id)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_enabled ON hsb_endpoints(enabled)",
            "CREATE INDEX IF NOT EXISTS idx_endpoints_lifecycle ON hsb_endpoints(lifecycle_status)",
            "CREATE INDEX IF NOT EXISTS idx_endpoint_versions_changed_at ON hsb_endpoint_versions(changed_at)",
            "CREATE INDEX IF NOT EXISTS idx_endpoint_status_updated_at ON hsb_endpoint_status(updated_at)",
            "CREATE INDEX IF NOT EXISTS idx_workflows_name ON hsb_workflows(name)",
            "CREATE INDEX IF NOT EXISTS idx_workflows_enabled ON hsb_workflows(enabled)",
            "CREATE INDEX IF NOT EXISTS idx_workflow_instances_workflow_id ON hsb_workflow_instances(workflow_id)",
            "CREATE INDEX IF NOT EXISTS idx_workflow_instances_status ON hsb_workflow_instances(status)",
            "CREATE INDEX IF NOT EXISTS idx_workflow_instances_created_at ON hsb_workflow_instances(created_at)",
        ] {
            sqlx::query(idx_sql)
                .execute(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Index creation failed: {}", e),
                })?;
        }

        info!("PostgreSQL migrations completed");
        Ok(())
    }

    /// 获取连接池引用
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    fn serialize_json<T: serde::Serialize>(&self, value: &T, field: &str) -> HsbResult<Value> {
        serde_json::to_value(value).map_err(|e| HsbError::SerializationError {
            message: format!("Failed to serialize {}: {}", field, e),
        })
    }

    fn parse_json<T: serde::de::DeserializeOwned>(
        &self,
        value: Value,
        field: &str,
    ) -> HsbResult<T> {
        serde_json::from_value(value).map_err(|e| HsbError::SerializationError {
            message: format!("Failed to deserialize {}: {}", field, e),
        })
    }

    fn row_try_get<T>(&self, row: &sqlx::postgres::PgRow, field: &str) -> HsbResult<T>
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
    {
        row.try_get(field).map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to decode column {}: {}", field, e),
        })
    }

    fn row_to_endpoint(&self, row: sqlx::postgres::PgRow) -> HsbResult<Endpoint> {
        let system_type = row
            .try_get::<String, _>("system_type")
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to decode column system_type: {}", e),
            })?
            .parse()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse endpoint system_type: {}", e),
            })?;
        let protocol = row
            .try_get::<String, _>("protocol")
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to decode column protocol: {}", e),
            })?
            .parse()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse endpoint protocol: {}", e),
            })?;
        let lifecycle_status = row
            .try_get::<String, _>("lifecycle_status")
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to decode column lifecycle_status: {}", e),
            })?
            .parse()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse endpoint lifecycle_status: {}", e),
            })?;

        Ok(Endpoint {
            id: self.row_try_get::<String>(&row, "id")?.into(),
            organization_id: OrganizationId::new(
                self.row_try_get::<String>(&row, "organization_id")?,
            ),
            system_id: SystemId::new(self.row_try_get::<String>(&row, "system_id")?),
            roles: self.parse_json(self.row_try_get(&row, "roles")?, "endpoint.roles")?,
            name: self.row_try_get(&row, "name")?,
            description: self.row_try_get(&row, "description")?,
            system_type,
            protocol,
            connection: self
                .parse_json(self.row_try_get(&row, "connection")?, "endpoint.connection")?,
            auth: self
                .row_try_get::<Option<Value>>(&row, "auth")?
                .map(|value| self.parse_json(value, "endpoint.auth"))
                .transpose()?,
            config: self.parse_json(self.row_try_get(&row, "config")?, "endpoint.config")?,
            enabled: self.row_try_get(&row, "enabled")?,
            lifecycle_status,
            version: self.row_try_get::<i32>(&row, "version")? as u32,
            security: self.parse_json(self.row_try_get(&row, "security")?, "endpoint.security")?,
            properties: self
                .parse_json(self.row_try_get(&row, "properties")?, "endpoint.properties")?,
            created_at: self.row_try_get(&row, "created_at")?,
            updated_at: self.row_try_get(&row, "updated_at")?,
            created_by: self.row_try_get(&row, "created_by")?,
            updated_by: self.row_try_get(&row, "updated_by")?,
        })
    }

    fn row_to_organization(&self, row: sqlx::postgres::PgRow) -> HsbResult<Organization> {
        let organization_type = self
            .row_try_get::<String>(&row, "organization_type")?
            .parse::<OrganizationType>()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse organization type: {}", e),
            })?;

        Ok(Organization {
            id: OrganizationId::new(self.row_try_get::<String>(&row, "id")?),
            name: self.row_try_get(&row, "name")?,
            description: self.row_try_get(&row, "description")?,
            organization_type,
            parent_organization_id: self
                .row_try_get::<Option<String>>(&row, "parent_organization_id")?
                .map(OrganizationId::new),
            enabled: self.row_try_get(&row, "enabled")?,
            properties: self.parse_json(
                self.row_try_get(&row, "properties")?,
                "organization.properties",
            )?,
            created_at: self.row_try_get(&row, "created_at")?,
            updated_at: self.row_try_get(&row, "updated_at")?,
        })
    }

    fn row_to_system(&self, row: sqlx::postgres::PgRow) -> HsbResult<IntegrationSystem> {
        let system_type = self
            .row_try_get::<String>(&row, "system_type")?
            .parse()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse system type: {}", e),
            })?;

        Ok(IntegrationSystem {
            id: SystemId::new(self.row_try_get::<String>(&row, "id")?),
            organization_id: OrganizationId::new(
                self.row_try_get::<String>(&row, "organization_id")?,
            ),
            name: self.row_try_get(&row, "name")?,
            description: self.row_try_get(&row, "description")?,
            system_type,
            topic_namespace: self.row_try_get(&row, "topic_namespace")?,
            topic_prefix: self.row_try_get(&row, "topic_prefix")?,
            enabled: self.row_try_get(&row, "enabled")?,
            properties: self.parse_json(
                self.row_try_get(&row, "properties")?,
                "integration_system.properties",
            )?,
            created_at: self.row_try_get(&row, "created_at")?,
            updated_at: self.row_try_get(&row, "updated_at")?,
        })
    }

    fn row_to_message(&self, row: sqlx::postgres::PgRow) -> HsbResult<Message> {
        let topic = self
            .row_try_get::<Option<String>>(&row, "topic")?
            .map(|value| {
                Topic::parse(&value).map_err(|e| HsbError::SerializationError {
                    message: format!("Failed to parse message topic: {}", e),
                })
            })
            .transpose()?;
        let protocol = self
            .row_try_get::<String>(&row, "protocol")?
            .parse::<ProtocolType>()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse message protocol: {}", e),
            })?;
        let status = parse_message_status(&self.row_try_get::<String>(&row, "status")?)?;
        let priority = self
            .row_try_get::<String>(&row, "priority")?
            .parse::<MessagePriority>()
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to parse message priority: {}", e),
            })?;
        let created_at: chrono::DateTime<chrono::Utc> = self.row_try_get(&row, "created_at")?;

        Ok(Message {
            id: self
                .row_try_get::<String>(&row, "id")?
                .parse()
                .map_err(|e| HsbError::SerializationError {
                    message: format!("Failed to parse message id: {}", e),
                })?,
            topic,
            timestamp: created_at.timestamp(),
            headers: self.parse_json(self.row_try_get(&row, "headers")?, "message.headers")?,
            payload: self.row_try_get(&row, "payload")?,
            meta: self.parse_json(self.row_try_get(&row, "meta")?, "message.meta")?,
            version: self.row_try_get::<i32>(&row, "version")? as u32,
            source_system: SystemId::new(self.row_try_get::<String>(&row, "source_system")?),
            target_system: self
                .row_try_get::<Option<String>>(&row, "target_system")?
                .map(SystemId::new),
            protocol,
            message_type: self.row_try_get(&row, "message_type")?,
            raw_payload: self.row_try_get(&row, "raw_payload")?,
            status,
            priority,
            trace_id: TraceId::from_string(self.row_try_get::<String>(&row, "trace_id")?),
            correlation_id: self.row_try_get(&row, "correlation_id")?,
            metadata: self.parse_json(self.row_try_get(&row, "metadata")?, "message.metadata")?,
            created_at,
            updated_at: self.row_try_get(&row, "updated_at")?,
        })
    }

    fn row_to_workflow_definition(
        &self,
        row: sqlx::postgres::PgRow,
    ) -> HsbResult<StoredWorkflowDefinition> {
        Ok(StoredWorkflowDefinition {
            workflow: self
                .parse_json(self.row_try_get(&row, "workflow")?, "workflow.definition")?,
            created_at: self.row_try_get(&row, "created_at")?,
            updated_at: self.row_try_get(&row, "updated_at")?,
        })
    }

    async fn insert_endpoint_version(
        &self,
        endpoint: &Endpoint,
        actor: Option<&str>,
        change_note: Option<&str>,
    ) -> HsbResult<()> {
        let snapshot = self.serialize_json(endpoint, "endpoint.version_snapshot")?;
        sqlx::query(
            r#"
            INSERT INTO hsb_endpoint_versions (endpoint_id, version, snapshot, changed_at, changed_by, change_note)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(endpoint.id.as_str())
        .bind(endpoint.version as i32)
        .bind(snapshot)
        .bind(endpoint.updated_at)
        .bind(actor)
        .bind(change_note)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to insert endpoint version: {}", e),
        })?;
        Ok(())
    }

    /// 保存死信记录
    pub async fn save_dead_letter(
        &self,
        id: &str,
        message_id: &str,
        reason: &str,
        error_detail: Option<&str>,
        retry_count: i32,
        source_route_id: Option<&str>,
        message_data: &serde_json::Value,
    ) -> HsbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO hsb_dead_letters (id, message_id, reason, error_detail, retry_count, source_route_id, message_data)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(id)
        .bind(message_id)
        .bind(reason)
        .bind(error_detail)
        .bind(retry_count)
        .bind(source_route_id)
        .bind(message_data)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save dead letter: {}", e),
        })?;
        Ok(())
    }

    /// 保存审计日志
    pub async fn save_audit_log(
        &self,
        id: &str,
        trace_id: Option<&str>,
        message_id: Option<&str>,
        event_type: &str,
        severity: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
        component: &str,
        description: &str,
        success: bool,
        error: Option<&str>,
        duration_ms: Option<i64>,
        metadata: &serde_json::Value,
    ) -> HsbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO hsb_audit_log (id, trace_id, message_id, event_type, severity, timestamp, component, description, success, error, duration_ms, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(id)
        .bind(trace_id)
        .bind(message_id)
        .bind(event_type)
        .bind(severity)
        .bind(timestamp)
        .bind(component)
        .bind(description)
        .bind(success)
        .bind(error)
        .bind(duration_ms)
        .bind(metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save audit log: {}", e),
        })?;
        Ok(())
    }

    // ---- 缓存操作（UNLOGGED 表 + JSONB，替代 Redis）----

    /// 设置缓存（带 TTL）
    pub async fn cache_set(
        &self,
        key: &str,
        value: &serde_json::Value,
        ttl_secs: Option<u64>,
    ) -> HsbResult<()> {
        let expires_at =
            ttl_secs.map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

        sqlx::query(
            r#"
            INSERT INTO hsb_cache (key, value, expires_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (key) DO UPDATE SET
                value = $2,
                expires_at = $3,
                created_at = NOW()
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to set cache: {}", e),
        })?;
        Ok(())
    }

    /// 获取缓存（自动过滤过期条目）
    pub async fn cache_get(&self, key: &str) -> HsbResult<Option<serde_json::Value>> {
        let row: Option<(serde_json::Value,)> = sqlx::query_as(
            "SELECT value FROM hsb_cache WHERE key = $1 AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to get cache: {}", e),
        })?;
        Ok(row.map(|(v,)| v))
    }

    /// 删除缓存
    pub async fn cache_del(&self, key: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_cache WHERE key = $1")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete cache: {}", e),
            })?;
        Ok(())
    }

    /// 按前缀删除缓存
    pub async fn cache_del_prefix(&self, prefix: &str) -> HsbResult<u64> {
        let result = sqlx::query("DELETE FROM hsb_cache WHERE key LIKE $1")
            .bind(format!("{}%", prefix))
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete cache by prefix: {}", e),
            })?;
        Ok(result.rows_affected())
    }

    /// 清理过期缓存
    pub async fn cache_cleanup(&self) -> HsbResult<u64> {
        let result = sqlx::query(
            "DELETE FROM hsb_cache WHERE expires_at IS NOT NULL AND expires_at <= NOW()",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to cleanup cache: {}", e),
        })?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl EndpointStore for PgStore {
    async fn create_endpoint(
        &self,
        endpoint: &Endpoint,
        actor: Option<&str>,
        change_note: Option<&str>,
    ) -> HsbResult<()> {
        if self.get_endpoint(endpoint.id.as_str()).await?.is_some() {
            return Err(HsbError::DuplicateRecord {
                entity: "Endpoint".to_string(),
                id: endpoint.id.to_string(),
            });
        }

        sqlx::query(
            r#"
            INSERT INTO hsb_endpoints (
                id, organization_id, system_id, name, description, system_type, protocol, roles,
                connection, auth, config, enabled, lifecycle_status, version, security,
                properties, created_at, updated_at, created_by, updated_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
            "#,
        )
        .bind(endpoint.id.as_str())
        .bind(endpoint.organization_id.as_str())
        .bind(endpoint.system_id.as_str())
        .bind(&endpoint.name)
        .bind(&endpoint.description)
        .bind(endpoint.system_type.name())
        .bind(endpoint.protocol.name())
        .bind(self.serialize_json(&endpoint.roles, "endpoint.roles")?)
        .bind(self.serialize_json(&endpoint.connection, "endpoint.connection")?)
        .bind(endpoint.auth.as_ref().map(|auth| self.serialize_json(auth, "endpoint.auth")).transpose()?)
        .bind(self.serialize_json(&endpoint.config, "endpoint.config")?)
        .bind(endpoint.enabled)
        .bind(format!("{:?}", endpoint.lifecycle_status).to_uppercase())
        .bind(endpoint.version as i32)
        .bind(self.serialize_json(&endpoint.security, "endpoint.security")?)
        .bind(self.serialize_json(&endpoint.properties, "endpoint.properties")?)
        .bind(endpoint.created_at)
        .bind(endpoint.updated_at)
        .bind(&endpoint.created_by)
        .bind(&endpoint.updated_by)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to create endpoint: {}", e),
        })?;

        self.insert_endpoint_version(endpoint, actor, change_note)
            .await?;
        self.upsert_endpoint_status(&default_endpoint_runtime_status(endpoint))
            .await?;
        Ok(())
    }

    async fn get_endpoint(&self, id: &str) -> HsbResult<Option<Endpoint>> {
        let row = sqlx::query(
            r#"
                 SELECT id, organization_id, system_id, name, description, system_type, protocol, roles, connection, auth, config,
                     enabled, lifecycle_status, version, security, properties,
                   created_at, updated_at, created_by, updated_by
            FROM hsb_endpoints
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to load endpoint: {}", e),
        })?;

        row.map(|row| self.row_to_endpoint(row)).transpose()
    }

    async fn list_endpoints(&self) -> HsbResult<Vec<Endpoint>> {
        let rows = sqlx::query(
            r#"
                 SELECT id, organization_id, system_id, name, description, system_type, protocol, roles, connection, auth, config,
                     enabled, lifecycle_status, version, security, properties,
                   created_at, updated_at, created_by, updated_by
            FROM hsb_endpoints
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to list endpoints: {}", e),
        })?;

        rows.into_iter()
            .map(|row| self.row_to_endpoint(row))
            .collect()
    }

    async fn update_endpoint(
        &self,
        endpoint: &Endpoint,
        actor: Option<&str>,
        change_note: Option<&str>,
    ) -> HsbResult<()> {
        let updated = sqlx::query(
            r#"
            UPDATE hsb_endpoints
            SET organization_id = $2,
                system_id = $3,
                name = $4,
                description = $5,
                system_type = $6,
                protocol = $7,
                roles = $8,
                connection = $9,
                auth = $10,
                config = $11,
                enabled = $12,
                lifecycle_status = $13,
                version = $14,
                security = $15,
                properties = $16,
                updated_at = $17,
                updated_by = $18
            WHERE id = $1
            "#,
        )
        .bind(endpoint.id.as_str())
        .bind(endpoint.organization_id.as_str())
        .bind(endpoint.system_id.as_str())
        .bind(&endpoint.name)
        .bind(&endpoint.description)
        .bind(endpoint.system_type.name())
        .bind(endpoint.protocol.name())
        .bind(self.serialize_json(&endpoint.roles, "endpoint.roles")?)
        .bind(self.serialize_json(&endpoint.connection, "endpoint.connection")?)
        .bind(
            endpoint
                .auth
                .as_ref()
                .map(|auth| self.serialize_json(auth, "endpoint.auth"))
                .transpose()?,
        )
        .bind(self.serialize_json(&endpoint.config, "endpoint.config")?)
        .bind(endpoint.enabled)
        .bind(format!("{:?}", endpoint.lifecycle_status).to_uppercase())
        .bind(endpoint.version as i32)
        .bind(self.serialize_json(&endpoint.security, "endpoint.security")?)
        .bind(self.serialize_json(&endpoint.properties, "endpoint.properties")?)
        .bind(endpoint.updated_at)
        .bind(&endpoint.updated_by)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to update endpoint: {}", e),
        })?;

        if updated.rows_affected() == 0 {
            return Err(HsbError::NotFound {
                entity: "Endpoint".to_string(),
                id: endpoint.id.to_string(),
            });
        }

        self.insert_endpoint_version(endpoint, actor, change_note)
            .await?;
        Ok(())
    }

    async fn delete_endpoint(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_endpoints WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete endpoint: {}", e),
            })?;
        Ok(())
    }

    async fn list_endpoint_versions(&self, id: &str) -> HsbResult<Vec<EndpointVersionRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT endpoint_id, version, snapshot, changed_at, changed_by, change_note
            FROM hsb_endpoint_versions
            WHERE endpoint_id = $1
            ORDER BY version DESC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to list endpoint versions: {}", e),
        })?;

        rows.into_iter()
            .map(|row| {
                let snapshot: Endpoint = self.parse_json(
                    self.row_try_get(&row, "snapshot")?,
                    "endpoint.version_snapshot",
                )?;
                Ok(EndpointVersionRecord {
                    endpoint_id: self.row_try_get(&row, "endpoint_id")?,
                    version: self.row_try_get::<i32>(&row, "version")? as u32,
                    snapshot,
                    changed_at: self.row_try_get(&row, "changed_at")?,
                    changed_by: self.row_try_get(&row, "changed_by")?,
                    change_note: self.row_try_get(&row, "change_note")?,
                })
            })
            .collect()
    }

    async fn get_endpoint_status(&self, id: &str) -> HsbResult<Option<EndpointRuntimeStatus>> {
        let row = sqlx::query(
            r#"
            SELECT endpoint_id, healthy, latency_ms, last_error, circuit_state,
                   consecutive_failures, last_check_at, last_delivery_at, updated_at
            FROM hsb_endpoint_status
            WHERE endpoint_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to load endpoint status: {}", e),
        })?;

        if let Some(row) = row {
            return Ok(Some(EndpointRuntimeStatus {
                endpoint_id: self.row_try_get(&row, "endpoint_id")?,
                healthy: self.row_try_get(&row, "healthy")?,
                latency_ms: self
                    .row_try_get::<Option<i64>>(&row, "latency_ms")?
                    .map(|value| value as u64),
                last_error: self.row_try_get(&row, "last_error")?,
                circuit_state: self.row_try_get(&row, "circuit_state")?,
                consecutive_failures: self.row_try_get::<i32>(&row, "consecutive_failures")? as u32,
                last_check_at: self.row_try_get(&row, "last_check_at")?,
                last_delivery_at: self.row_try_get(&row, "last_delivery_at")?,
                updated_at: self.row_try_get(&row, "updated_at")?,
            }));
        }

        if let Some(endpoint) = self.get_endpoint(id).await? {
            let status = default_endpoint_runtime_status(&endpoint);
            self.upsert_endpoint_status(&status).await?;
            return Ok(Some(status));
        }

        Ok(None)
    }

    async fn upsert_endpoint_status(&self, status: &EndpointRuntimeStatus) -> HsbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO hsb_endpoint_status (
                endpoint_id, healthy, latency_ms, last_error, circuit_state,
                consecutive_failures, last_check_at, last_delivery_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (endpoint_id) DO UPDATE SET
                healthy = EXCLUDED.healthy,
                latency_ms = EXCLUDED.latency_ms,
                last_error = EXCLUDED.last_error,
                circuit_state = EXCLUDED.circuit_state,
                consecutive_failures = EXCLUDED.consecutive_failures,
                last_check_at = EXCLUDED.last_check_at,
                last_delivery_at = EXCLUDED.last_delivery_at,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&status.endpoint_id)
        .bind(status.healthy)
        .bind(status.latency_ms.map(|value| value as i64))
        .bind(&status.last_error)
        .bind(&status.circuit_state)
        .bind(status.consecutive_failures as i32)
        .bind(status.last_check_at)
        .bind(status.last_delivery_at)
        .bind(status.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to upsert endpoint status: {}", e),
        })?;
        Ok(())
    }
}

#[async_trait]
impl OrganizationStore for PgStore {
    async fn create_organization(&self, organization: &Organization) -> HsbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO hsb_organizations (
                id, name, description, organization_type, parent_organization_id,
                enabled, properties, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(organization.id.as_str())
        .bind(&organization.name)
        .bind(&organization.description)
        .bind(organization.organization_type.name())
        .bind(
            organization
                .parent_organization_id
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind(organization.enabled)
        .bind(self.serialize_json(&organization.properties, "organization.properties")?)
        .bind(organization.created_at)
        .bind(organization.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to create organization: {}", e),
        })?;
        Ok(())
    }

    async fn get_organization(&self, id: &str) -> HsbResult<Option<Organization>> {
        let row = sqlx::query(
            "SELECT id, name, description, organization_type, parent_organization_id, enabled, properties, created_at, updated_at FROM hsb_organizations WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to load organization: {}", e),
        })?;

        row.map(|row| self.row_to_organization(row)).transpose()
    }

    async fn list_organizations(&self) -> HsbResult<Vec<Organization>> {
        let rows = sqlx::query(
            "SELECT id, name, description, organization_type, parent_organization_id, enabled, properties, created_at, updated_at FROM hsb_organizations ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to list organizations: {}", e),
        })?;

        rows.into_iter()
            .map(|row| self.row_to_organization(row))
            .collect()
    }

    async fn update_organization(&self, organization: &Organization) -> HsbResult<()> {
        sqlx::query(
            r#"
            UPDATE hsb_organizations
            SET name = $2,
                description = $3,
                organization_type = $4,
                parent_organization_id = $5,
                enabled = $6,
                properties = $7,
                updated_at = $8
            WHERE id = $1
            "#,
        )
        .bind(organization.id.as_str())
        .bind(&organization.name)
        .bind(&organization.description)
        .bind(organization.organization_type.name())
        .bind(
            organization
                .parent_organization_id
                .as_ref()
                .map(|value| value.to_string()),
        )
        .bind(organization.enabled)
        .bind(self.serialize_json(&organization.properties, "organization.properties")?)
        .bind(organization.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to update organization: {}", e),
        })?;
        Ok(())
    }

    async fn delete_organization(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_organizations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete organization: {}", e),
            })?;
        Ok(())
    }
}

#[async_trait]
impl IntegrationSystemStore for PgStore {
    async fn create_system(&self, system: &IntegrationSystem) -> HsbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO hsb_integration_systems (
                id, organization_id, name, description, system_type,
                topic_namespace, topic_prefix, enabled, properties, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(system.id.as_str())
        .bind(system.organization_id.as_str())
        .bind(&system.name)
        .bind(&system.description)
        .bind(system.system_type.name())
        .bind(&system.topic_namespace)
        .bind(&system.topic_prefix)
        .bind(system.enabled)
        .bind(self.serialize_json(&system.properties, "integration_system.properties")?)
        .bind(system.created_at)
        .bind(system.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to create system: {}", e),
        })?;
        Ok(())
    }

    async fn get_system(&self, id: &str) -> HsbResult<Option<IntegrationSystem>> {
        let row = sqlx::query(
            "SELECT id, organization_id, name, description, system_type, topic_namespace, topic_prefix, enabled, properties, created_at, updated_at FROM hsb_integration_systems WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to load system: {}", e),
        })?;

        row.map(|row| self.row_to_system(row)).transpose()
    }

    async fn list_systems(&self) -> HsbResult<Vec<IntegrationSystem>> {
        let rows = sqlx::query(
            "SELECT id, organization_id, name, description, system_type, topic_namespace, topic_prefix, enabled, properties, created_at, updated_at FROM hsb_integration_systems ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to list systems: {}", e),
        })?;

        rows.into_iter()
            .map(|row| self.row_to_system(row))
            .collect()
    }

    async fn update_system(&self, system: &IntegrationSystem) -> HsbResult<()> {
        sqlx::query(
            r#"
            UPDATE hsb_integration_systems
            SET organization_id = $2,
                name = $3,
                description = $4,
                system_type = $5,
                topic_namespace = $6,
                topic_prefix = $7,
                enabled = $8,
                properties = $9,
                updated_at = $10
            WHERE id = $1
            "#,
        )
        .bind(system.id.as_str())
        .bind(system.organization_id.as_str())
        .bind(&system.name)
        .bind(&system.description)
        .bind(system.system_type.name())
        .bind(&system.topic_namespace)
        .bind(&system.topic_prefix)
        .bind(system.enabled)
        .bind(self.serialize_json(&system.properties, "integration_system.properties")?)
        .bind(system.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to update system: {}", e),
        })?;
        Ok(())
    }

    async fn delete_system(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_integration_systems WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete system: {}", e),
            })?;
        Ok(())
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

fn message_status_to_db(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Received => "RECEIVED",
        MessageStatus::Processing => "PROCESSING",
        MessageStatus::Routing => "ROUTING",
        MessageStatus::Transforming => "TRANSFORMING",
        MessageStatus::Delivering => "DELIVERING",
        MessageStatus::Delivered => "DELIVERED",
        MessageStatus::RetryPending => "RETRY_PENDING",
        MessageStatus::Retrying => "RETRYING",
        MessageStatus::DeadLetter => "DEAD_LETTER",
        MessageStatus::Completed => "COMPLETED",
        MessageStatus::Failed => "FAILED",
        MessageStatus::Compensated => "COMPENSATED",
    }
}

fn parse_message_status(value: &str) -> HsbResult<MessageStatus> {
    match value.to_ascii_uppercase().as_str() {
        "RECEIVED" => Ok(MessageStatus::Received),
        "PROCESSING" => Ok(MessageStatus::Processing),
        "ROUTING" => Ok(MessageStatus::Routing),
        "TRANSFORMING" => Ok(MessageStatus::Transforming),
        "DELIVERING" => Ok(MessageStatus::Delivering),
        "DELIVERED" => Ok(MessageStatus::Delivered),
        "RETRY_PENDING" => Ok(MessageStatus::RetryPending),
        "RETRYING" => Ok(MessageStatus::Retrying),
        "DEAD_LETTER" | "DEADLETTER" => Ok(MessageStatus::DeadLetter),
        "COMPLETED" => Ok(MessageStatus::Completed),
        "FAILED" => Ok(MessageStatus::Failed),
        "COMPENSATED" => Ok(MessageStatus::Compensated),
        _ => Err(HsbError::SerializationError {
            message: format!("Failed to parse message status: {}", value),
        }),
    }
}

#[async_trait]
impl PersistentMessageStore for PgStore {
    async fn save_message(&self, msg: &Message) -> HsbResult<()> {
        let headers = serde_json::to_value(&msg.headers).unwrap_or_default();
        let meta = serde_json::to_value(&msg.meta).unwrap_or_default();
        let metadata = serde_json::to_value(&msg.metadata).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO hsb_messages (id, topic, version, source_system, target_system, protocol, message_type, headers, payload, raw_payload, meta, status, priority, trace_id, correlation_id, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            ON CONFLICT (id) DO UPDATE SET
                topic = EXCLUDED.topic,
                version = EXCLUDED.version,
                target_system = EXCLUDED.target_system,
                protocol = EXCLUDED.protocol,
                message_type = EXCLUDED.message_type,
                headers = EXCLUDED.headers,
                payload = EXCLUDED.payload,
                raw_payload = EXCLUDED.raw_payload,
                meta = EXCLUDED.meta,
                status = EXCLUDED.status,
                priority = EXCLUDED.priority,
                trace_id = EXCLUDED.trace_id,
                correlation_id = EXCLUDED.correlation_id,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(msg.id.to_string())
        .bind(msg.topic.as_ref().map(|value| value.as_str().to_string()))
        .bind(msg.version as i32)
        .bind(msg.source_system.to_string())
        .bind(msg.target_system.as_ref().map(|s| s.to_string()))
        .bind(msg.protocol.to_string())
        .bind(&msg.message_type)
        .bind(&headers)
        .bind(&msg.payload)
        .bind(&msg.raw_payload)
        .bind(&meta)
        .bind(message_status_to_db(msg.status))
        .bind(msg.priority.to_string())
        .bind(msg.trace_id.to_string())
        .bind(&msg.correlation_id)
        .bind(&metadata)
        .bind(msg.created_at)
        .bind(msg.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save message: {}", e),
        })?;

        Ok(())
    }

    async fn list_messages(&self, query: &PersistentMessageQuery) -> HsbResult<Vec<Message>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, topic, version, source_system, target_system, protocol, message_type, headers, payload, raw_payload, meta, status, priority, trace_id, correlation_id, metadata, created_at, updated_at FROM hsb_messages WHERE 1 = 1",
        );

        if let Some(source_system) = &query.source_system {
            builder
                .push(" AND source_system = ")
                .push_bind(source_system);
        }
        if let Some(target_system) = &query.target_system {
            builder
                .push(" AND target_system = ")
                .push_bind(target_system);
        }
        if let Some(message_type) = &query.message_type {
            builder.push(" AND message_type = ").push_bind(message_type);
        }
        if let Some(status) = &query.status {
            builder
                .push(" AND status = ")
                .push_bind(status.to_ascii_uppercase());
        }
        if let Some(from_time) = query.from_time {
            builder.push(" AND created_at >= ").push_bind(from_time);
        }
        if let Some(to_time) = query.to_time {
            builder.push(" AND created_at <= ").push_bind(to_time);
        }

        builder.push(" ORDER BY created_at DESC");
        builder
            .push(" LIMIT ")
            .push_bind(query.limit.unwrap_or(100) as i64)
            .push(" OFFSET ")
            .push_bind(query.offset.unwrap_or(0) as i64);

        let rows =
            builder
                .build()
                .fetch_all(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Failed to list messages: {}", e),
                })?;

        rows.into_iter()
            .map(|row| self.row_to_message(row))
            .collect()
    }

    async fn get_message(&self, id: &str) -> HsbResult<Option<Message>> {
        let row = sqlx::query(
            r#"
            SELECT id, topic, version, source_system, target_system, protocol, message_type, headers, payload, raw_payload, meta, status, priority, trace_id, correlation_id, metadata, created_at, updated_at
            FROM hsb_messages
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to get message: {}", e),
        })?;

        row.map(|row| self.row_to_message(row)).transpose()
    }

    async fn delete_message(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_messages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete message: {}", e),
            })?;
        Ok(())
    }

    async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>> {
        let rows = sqlx::query(
            r#"
            SELECT id, topic, version, source_system, target_system, protocol, message_type, headers, payload, raw_payload, meta, status, priority, trace_id, correlation_id, metadata, created_at, updated_at
            FROM hsb_messages
            WHERE status = ANY($1)
            ORDER BY created_at ASC
            LIMIT $2
            "#,
        )
        .bind(vec![
            message_status_to_db(MessageStatus::Received),
            message_status_to_db(MessageStatus::Processing),
            message_status_to_db(MessageStatus::Routing),
            message_status_to_db(MessageStatus::Transforming),
            message_status_to_db(MessageStatus::Delivering),
            message_status_to_db(MessageStatus::RetryPending),
            message_status_to_db(MessageStatus::Retrying),
        ])
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to query pending messages: {}", e),
        })?;

        rows.into_iter()
            .map(|row| self.row_to_message(row))
            .collect()
    }

    async fn save_batch(&self, messages: &[Message]) -> HsbResult<()> {
        // 使用事务批量保存
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to begin transaction: {}", e),
            })?;

        for msg in messages {
            let headers = serde_json::to_value(&msg.headers).unwrap_or_default();
            let meta = serde_json::to_value(&msg.meta).unwrap_or_default();
            let metadata = serde_json::to_value(&msg.metadata).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO hsb_messages (id, topic, version, source_system, target_system, protocol, message_type, headers, payload, raw_payload, meta, status, priority, trace_id, correlation_id, metadata, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                ON CONFLICT (id) DO UPDATE SET
                    topic = EXCLUDED.topic,
                    version = EXCLUDED.version,
                    target_system = EXCLUDED.target_system,
                    protocol = EXCLUDED.protocol,
                    message_type = EXCLUDED.message_type,
                    headers = EXCLUDED.headers,
                    payload = EXCLUDED.payload,
                    raw_payload = EXCLUDED.raw_payload,
                    meta = EXCLUDED.meta,
                    status = EXCLUDED.status,
                    priority = EXCLUDED.priority,
                    trace_id = EXCLUDED.trace_id,
                    correlation_id = EXCLUDED.correlation_id,
                    metadata = EXCLUDED.metadata,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(msg.id.to_string())
            .bind(msg.topic.as_ref().map(|value| value.as_str().to_string()))
            .bind(msg.version as i32)
            .bind(msg.source_system.to_string())
            .bind(msg.target_system.as_ref().map(|s| s.to_string()))
            .bind(msg.protocol.to_string())
            .bind(&msg.message_type)
            .bind(&headers)
            .bind(&msg.payload)
            .bind(&msg.raw_payload)
            .bind(&meta)
            .bind(message_status_to_db(msg.status))
            .bind(msg.priority.to_string())
            .bind(msg.trace_id.to_string())
            .bind(&msg.correlation_id)
            .bind(&metadata)
            .bind(msg.created_at)
            .bind(msg.updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to save message in batch: {}", e),
            })?;
        }

        tx.commit().await.map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to commit batch: {}", e),
        })?;

        Ok(())
    }
}

#[async_trait]
impl RouteStore for PgStore {
    async fn save_route(&self, route: &crate::Route) -> HsbResult<()> {
        let config = serde_json::to_value(route).map_err(|e| HsbError::SerializationError {
            message: format!("Failed to serialize route: {}", e),
        })?;

        sqlx::query(
            r#"
            INSERT INTO hsb_routes (id, name, config, enabled, priority, updated_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (id) DO UPDATE SET
                name = $2,
                config = $3,
                enabled = $4,
                priority = $5,
                updated_at = NOW()
            "#,
        )
        .bind(route.id.to_string())
        .bind(&route.name)
        .bind(&config)
        .bind(route.enabled)
        .bind(route.priority)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save route: {}", e),
        })?;

        Ok(())
    }

    async fn get_route(&self, id: &str) -> HsbResult<Option<crate::Route>> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT config FROM hsb_routes WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Failed to get route: {}", e),
                })?;

        match row {
            Some((config,)) => {
                let route: crate::Route =
                    serde_json::from_value(config).map_err(|e| HsbError::ParseError {
                        message: format!("Failed to deserialize route: {}", e),
                    })?;
                Ok(Some(route))
            }
            None => Ok(None),
        }
    }

    async fn list_routes(&self) -> HsbResult<Vec<crate::Route>> {
        let rows: Vec<(serde_json::Value,)> =
            sqlx::query_as("SELECT config FROM hsb_routes ORDER BY priority DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Failed to list routes: {}", e),
                })?;

        let routes = rows
            .into_iter()
            .filter_map(|(config,)| serde_json::from_value(config).ok())
            .collect();

        Ok(routes)
    }

    async fn delete_route(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_routes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete route: {}", e),
            })?;
        Ok(())
    }
}

#[async_trait]
impl WorkflowStore for PgStore {
    async fn save_workflow(&self, workflow: &Workflow) -> HsbResult<()> {
        let workflow_json = self.serialize_json(workflow, "workflow.definition")?;

        sqlx::query(
            r#"
            INSERT INTO hsb_workflows (id, name, description, version, enabled, workflow, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                description = EXCLUDED.description,
                version = EXCLUDED.version,
                enabled = EXCLUDED.enabled,
                workflow = EXCLUDED.workflow,
                updated_at = NOW()
            "#,
        )
        .bind(&workflow.id)
        .bind(&workflow.name)
        .bind(&workflow.description)
        .bind(workflow.version as i32)
        .bind(workflow.enabled)
        .bind(&workflow_json)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save workflow: {}", e),
        })?;

        Ok(())
    }

    async fn get_workflow(&self, id: &str) -> HsbResult<Option<StoredWorkflowDefinition>> {
        let row = sqlx::query(
            r#"
            SELECT workflow, created_at, updated_at
            FROM hsb_workflows
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to get workflow: {}", e),
        })?;

        row.map(|row| self.row_to_workflow_definition(row))
            .transpose()
    }

    async fn list_workflows(&self) -> HsbResult<Vec<StoredWorkflowDefinition>> {
        let rows = sqlx::query(
            r#"
            SELECT workflow, created_at, updated_at
            FROM hsb_workflows
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to list workflows: {}", e),
        })?;

        rows.into_iter()
            .map(|row| self.row_to_workflow_definition(row))
            .collect()
    }

    async fn delete_workflow(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_workflows WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete workflow: {}", e),
            })?;
        Ok(())
    }

    async fn save_workflow_instance(&self, instance: &WorkflowInstance) -> HsbResult<()> {
        let instance_json = self.serialize_json(instance, "workflow.instance")?;

        sqlx::query(
            r#"
            INSERT INTO hsb_workflow_instances (
                id, workflow_id, workflow_version, status, current_step_id, instance_data,
                created_at, updated_at, completed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id) DO UPDATE SET
                workflow_id = EXCLUDED.workflow_id,
                workflow_version = EXCLUDED.workflow_version,
                status = EXCLUDED.status,
                current_step_id = EXCLUDED.current_step_id,
                instance_data = EXCLUDED.instance_data,
                updated_at = EXCLUDED.updated_at,
                completed_at = EXCLUDED.completed_at
            "#,
        )
        .bind(instance.id.to_string())
        .bind(&instance.workflow_id)
        .bind(instance.workflow_version as i32)
        .bind(format!("{:?}", instance.status))
        .bind(&instance.current_step_id)
        .bind(&instance_json)
        .bind(instance.created_at)
        .bind(instance.updated_at)
        .bind(instance.completed_at)
        .execute(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to save workflow instance: {}", e),
        })?;

        Ok(())
    }

    async fn get_workflow_instance(&self, id: &str) -> HsbResult<Option<WorkflowInstance>> {
        let row = sqlx::query(
            r#"
            SELECT instance_data
            FROM hsb_workflow_instances
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to get workflow instance: {}", e),
        })?;

        row.map(|row| {
            let value = row
                .try_get("instance_data")
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Failed to decode workflow instance column: {}", e),
                })?;
            self.parse_json(value, "workflow.instance")
        })
        .transpose()
    }

    async fn list_workflow_instances(
        &self,
        query: &WorkflowInstanceQuery,
    ) -> HsbResult<Vec<WorkflowInstance>> {
        let mut sql = QueryBuilder::<Postgres>::new(
            "SELECT instance_data FROM hsb_workflow_instances WHERE 1 = 1",
        );

        if let Some(workflow_id) = &query.workflow_id {
            sql.push(" AND workflow_id = ").push_bind(workflow_id);
        }
        if let Some(status) = &query.status {
            sql.push(" AND status = ")
                .push_bind(status.to_ascii_uppercase());
        }

        sql.push(" ORDER BY created_at DESC");

        if let Some(limit) = query.limit {
            sql.push(" LIMIT ").push_bind(limit as i64);
        }
        if let Some(offset) = query.offset {
            sql.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows =
            sql.build()
                .fetch_all(&self.pool)
                .await
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Failed to list workflow instances: {}", e),
                })?;

        rows.into_iter()
            .map(|row| {
                let value = row
                    .try_get("instance_data")
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Failed to decode workflow instance column: {}", e),
                    })?;
                self.parse_json(value, "workflow.instance")
            })
            .collect()
    }

    async fn delete_workflow_instance(&self, id: &str) -> HsbResult<()> {
        sqlx::query("DELETE FROM hsb_workflow_instances WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Failed to delete workflow instance: {}", e),
            })?;
        Ok(())
    }
}
