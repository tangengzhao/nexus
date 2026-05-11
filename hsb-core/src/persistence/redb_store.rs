//! Redb 本地嵌入式存储
//!
//! 用于：
//! - 消息本地缓存 / WAL
//! - 幂等键去重表（ExactlyOnce 语义）
//! - Circuit Breaker 状态持久化
//! - JetStream Consumer 的 offset 恢复

use crate::Message;
use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use redb::{Database, ReadableTable, TableDefinition};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

use super::{IdempotencyStore, PersistentMessageQuery, PersistentMessageStore};

// 表定义
const MESSAGES_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("messages");
const IDEMPOTENCY_TABLE: TableDefinition<&str, u64> = TableDefinition::new("idempotency");
const STATE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("state");

/// Redb 嵌入式存储
pub struct RedbStore {
    db: Arc<Database>,
}

impl RedbStore {
    /// 打开或创建 Redb 数据库
    pub fn open(path: impl AsRef<Path>) -> HsbResult<Self> {
        // 确保父目录存在
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| HsbError::InternalError {
                message: format!("Failed to create data directory: {}", e),
            })?;
        }

        let db = Database::create(path).map_err(|e| HsbError::DatabaseError {
            message: format!("Failed to open Redb: {}", e),
        })?;

        // 初始化表
        let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
            message: format!("Redb write txn failed: {}", e),
        })?;
        {
            let _ = write_txn.open_table(MESSAGES_TABLE);
            let _ = write_txn.open_table(IDEMPOTENCY_TABLE);
            let _ = write_txn.open_table(STATE_TABLE);
        }
        write_txn.commit().map_err(|e| HsbError::DatabaseError {
            message: format!("Redb commit failed: {}", e),
        })?;

        info!("Redb store initialized");

        Ok(Self { db: Arc::new(db) })
    }

    /// 保存任意状态数据（用于 Circuit Breaker 等组件）
    pub fn save_state(&self, key: &str, value: &[u8]) -> HsbResult<()> {
        let write_txn = self.db.begin_write().map_err(|e| HsbError::DatabaseError {
            message: format!("Redb write txn failed: {}", e),
        })?;
        {
            let mut table =
                write_txn
                    .open_table(STATE_TABLE)
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    })?;
            table
                .insert(key, value)
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Redb insert failed: {}", e),
                })?;
        }
        write_txn.commit().map_err(|e| HsbError::DatabaseError {
            message: format!("Redb commit failed: {}", e),
        })?;
        Ok(())
    }

    /// 读取状态数据
    pub fn get_state(&self, key: &str) -> HsbResult<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read().map_err(|e| HsbError::DatabaseError {
            message: format!("Redb read txn failed: {}", e),
        })?;
        let table = read_txn
            .open_table(STATE_TABLE)
            .map_err(|e| HsbError::DatabaseError {
                message: format!("Redb open table failed: {}", e),
            })?;
        let result = table.get(key).map_err(|e| HsbError::DatabaseError {
            message: format!("Redb get failed: {}", e),
        })?;
        Ok(result.map(|v| v.value().to_vec()))
    }
}

#[async_trait]
impl PersistentMessageStore for RedbStore {
    async fn save_message(&self, msg: &Message) -> HsbResult<()> {
        let id = msg.id.to_string();
        let data = serde_json::to_vec(msg).map_err(|e| HsbError::SerializationError {
            message: format!("Failed to serialize message: {}", e),
        })?;

        let db = self.db.clone();
        let id_clone = id.clone();
        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            {
                let mut table =
                    write_txn
                        .open_table(MESSAGES_TABLE)
                        .map_err(|e| HsbError::DatabaseError {
                            message: format!("Redb open table failed: {}", e),
                        })?;
                table
                    .insert(id_clone.as_str(), data.as_slice())
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb insert failed: {}", e),
                    })?;
            }
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok::<(), HsbError>(())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })??;
        Ok(())
    }

    async fn list_messages(&self, query: &PersistentMessageQuery) -> HsbResult<Vec<Message>> {
        let db = self.db.clone();
        let query = query.clone();
        tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb read txn failed: {}", e),
            })?;
            let table =
                read_txn
                    .open_table(MESSAGES_TABLE)
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    })?;

            let mut messages = Vec::new();
            let iter = table.iter().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb iter failed: {}", e),
            })?;

            for entry in iter {
                let (_, value) = entry.map_err(|e| HsbError::DatabaseError {
                    message: format!("Redb iter entry failed: {}", e),
                })?;
                let msg = match serde_json::from_slice::<Message>(value.value()) {
                    Ok(msg) => msg,
                    Err(_) => continue,
                };

                if let Some(source_system) = &query.source_system {
                    if msg.source_system.as_str() != source_system {
                        continue;
                    }
                }
                if let Some(target_system) = &query.target_system {
                    if msg.target_system.as_ref().map(|value| value.as_str())
                        != Some(target_system.as_str())
                    {
                        continue;
                    }
                }
                if let Some(message_type) = &query.message_type {
                    if msg.message_type.as_deref() != Some(message_type.as_str()) {
                        continue;
                    }
                }
                if let Some(status) = &query.status {
                    if !format!("{:?}", msg.status).eq_ignore_ascii_case(status) {
                        continue;
                    }
                }
                if let Some(from_time) = query.from_time {
                    if msg.created_at < from_time {
                        continue;
                    }
                }
                if let Some(to_time) = query.to_time {
                    if msg.created_at > to_time {
                        continue;
                    }
                }

                messages.push(msg);
            }

            messages.sort_by(|left, right| right.created_at.cmp(&left.created_at));

            let offset = query.offset.unwrap_or(0);
            let limit = query.limit.unwrap_or(messages.len());

            Ok(messages.into_iter().skip(offset).take(limit).collect())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }

    async fn get_message(&self, id: &str) -> HsbResult<Option<Message>> {
        let db = self.db.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb read txn failed: {}", e),
            })?;
            let table =
                read_txn
                    .open_table(MESSAGES_TABLE)
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    })?;
            match table.get(id.as_str()) {
                Ok(Some(value)) => {
                    let msg: Message = serde_json::from_slice(value.value()).map_err(|e| {
                        HsbError::ParseError {
                            message: format!("Failed to deserialize message: {}", e),
                        }
                    })?;
                    Ok(Some(msg))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(HsbError::DatabaseError {
                    message: format!("Redb get failed: {}", e),
                }),
            }
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }

    async fn delete_message(&self, id: &str) -> HsbResult<()> {
        let db = self.db.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            {
                let mut table =
                    write_txn
                        .open_table(MESSAGES_TABLE)
                        .map_err(|e| HsbError::DatabaseError {
                            message: format!("Redb open table failed: {}", e),
                        })?;
                table
                    .remove(id.as_str())
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb remove failed: {}", e),
                    })?;
            }
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok::<(), HsbError>(())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })??;
        Ok(())
    }

    async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>> {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb read txn failed: {}", e),
            })?;
            let table =
                read_txn
                    .open_table(MESSAGES_TABLE)
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    })?;

            let mut messages = Vec::new();
            let iter = table.iter().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb iter failed: {}", e),
            })?;

            for entry in iter {
                if messages.len() >= limit {
                    break;
                }
                let (_, value) = entry.map_err(|e| HsbError::DatabaseError {
                    message: format!("Redb iter entry failed: {}", e),
                })?;
                if let Ok(msg) = serde_json::from_slice::<Message>(value.value()) {
                    if !msg.status.is_terminal() {
                        messages.push(msg);
                    }
                }
            }

            Ok(messages)
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }

    async fn save_batch(&self, messages: &[Message]) -> HsbResult<()> {
        let db = self.db.clone();
        let serialized: Vec<(String, Vec<u8>)> = messages
            .iter()
            .map(|msg| {
                let id = msg.id.to_string();
                let data = serde_json::to_vec(msg).map_err(|e| HsbError::SerializationError {
                    message: format!("Failed to serialize message: {}", e),
                })?;
                Ok((id, data))
            })
            .collect::<HsbResult<Vec<_>>>()?;

        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            {
                let mut table =
                    write_txn
                        .open_table(MESSAGES_TABLE)
                        .map_err(|e| HsbError::DatabaseError {
                            message: format!("Redb open table failed: {}", e),
                        })?;
                for (id, data) in &serialized {
                    table.insert(id.as_str(), data.as_slice()).map_err(|e| {
                        HsbError::DatabaseError {
                            message: format!("Redb insert failed: {}", e),
                        }
                    })?;
                }
            }
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok::<(), HsbError>(())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })??;

        Ok(())
    }
}

#[async_trait]
impl IdempotencyStore for RedbStore {
    async fn check_and_mark(&self, idempotency_key: &str, _ttl_secs: u64) -> HsbResult<bool> {
        let db = self.db.clone();
        let key = idempotency_key.to_string();
        let now = chrono::Utc::now().timestamp() as u64;

        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            let is_new = {
                let mut table = write_txn.open_table(IDEMPOTENCY_TABLE).map_err(|e| {
                    HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    }
                })?;
                let exists = table
                    .get(key.as_str())
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb get failed: {}", e),
                    })?
                    .is_some();
                if exists {
                    false
                } else {
                    table
                        .insert(key.as_str(), now)
                        .map_err(|e| HsbError::DatabaseError {
                            message: format!("Redb insert failed: {}", e),
                        })?;
                    true
                }
            };
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok(is_new)
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }

    async fn is_processed(&self, idempotency_key: &str) -> HsbResult<bool> {
        let db = self.db.clone();
        let key = idempotency_key.to_string();

        tokio::task::spawn_blocking(move || {
            let read_txn = db.begin_read().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb read txn failed: {}", e),
            })?;
            let table =
                read_txn
                    .open_table(IDEMPOTENCY_TABLE)
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    })?;
            let exists = table
                .get(key.as_str())
                .map_err(|e| HsbError::DatabaseError {
                    message: format!("Redb get failed: {}", e),
                })?;
            Ok(exists.is_some())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }

    async fn clear_mark(&self, idempotency_key: &str) -> HsbResult<()> {
        let db = self.db.clone();
        let key = idempotency_key.to_string();

        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            {
                let mut table = write_txn.open_table(IDEMPOTENCY_TABLE).map_err(|e| {
                    HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    }
                })?;
                table
                    .remove(key.as_str())
                    .map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb remove failed: {}", e),
                    })?;
            }
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok::<(), HsbError>(())
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })??;

        Ok(())
    }

    async fn cleanup_expired(&self) -> HsbResult<u64> {
        let db = self.db.clone();
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(24)).timestamp() as u64;

        tokio::task::spawn_blocking(move || {
            let write_txn = db.begin_write().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb write txn failed: {}", e),
            })?;
            let mut removed = 0u64;
            {
                let mut table = write_txn.open_table(IDEMPOTENCY_TABLE).map_err(|e| {
                    HsbError::DatabaseError {
                        message: format!("Redb open table failed: {}", e),
                    }
                })?;

                // 收集过期的 key
                let expired_keys: Vec<String> = {
                    let iter = table.iter().map_err(|e| HsbError::DatabaseError {
                        message: format!("Redb iter failed: {}", e),
                    })?;
                    iter.filter_map(|entry| {
                        let (key, value) = entry.ok()?;
                        if value.value() < cutoff {
                            Some(key.value().to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
                };

                for key in &expired_keys {
                    table.remove(key.as_str()).ok();
                    removed += 1;
                }
            }
            write_txn.commit().map_err(|e| HsbError::DatabaseError {
                message: format!("Redb commit failed: {}", e),
            })?;
            Ok(removed)
        })
        .await
        .map_err(|e| HsbError::InternalError {
            message: format!("Spawn blocking failed: {}", e),
        })?
    }
}
