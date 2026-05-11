//! 消息队列实现

use crate::Message;
use async_trait::async_trait;
use hsb_common::{HsbError, HsbResult};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore, mpsc};

use super::{MessageMeta, MessageStatus, MessageStore};

/// 内存消息队列
#[allow(dead_code)]
pub struct InMemoryQueue {
    queue: Arc<RwLock<VecDeque<QueuedMessage>>>,
    max_size: usize,
    semaphore: Arc<Semaphore>,
    tx: mpsc::Sender<QueuedMessage>,
    rx: Arc<RwLock<mpsc::Receiver<QueuedMessage>>>,
}

/// 队列中的消息
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub message: Message,
    pub meta: MessageMeta,
}

impl InMemoryQueue {
    pub fn new(max_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(max_size);
        Self {
            queue: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
            semaphore: Arc::new(Semaphore::new(max_size)),
            tx,
            rx: Arc::new(RwLock::new(rx)),
        }
    }

    /// 入队
    pub async fn enqueue(&self, msg: Message) -> HsbResult<()> {
        let queue = self.queue.read().await;
        if queue.len() >= self.max_size {
            return Err(HsbError::QueueError {
                message: format!("Queue 'in_memory' is full (max_size: {})", self.max_size),
            });
        }
        drop(queue);

        let queued = QueuedMessage {
            meta: MessageMeta::new(&msg),
            message: msg,
        };

        self.tx
            .send(queued.clone())
            .await
            .map_err(|_| HsbError::InternalError {
                message: "Failed to enqueue message".to_string(),
            })?;

        let mut queue = self.queue.write().await;
        queue.push_back(queued);

        Ok(())
    }

    /// 出队
    pub async fn dequeue(&self) -> HsbResult<Option<QueuedMessage>> {
        let mut queue = self.queue.write().await;
        Ok(queue.pop_front())
    }

    /// 批量出队
    pub async fn dequeue_batch(&self, count: usize) -> HsbResult<Vec<QueuedMessage>> {
        let mut queue = self.queue.write().await;
        let mut result = Vec::with_capacity(count);

        for _ in 0..count {
            match queue.pop_front() {
                Some(msg) => result.push(msg),
                None => break,
            }
        }

        Ok(result)
    }

    /// 查看队首（不出队）
    pub async fn peek(&self) -> Option<QueuedMessage> {
        let queue = self.queue.read().await;
        queue.front().cloned()
    }

    /// 队列长度
    pub async fn len(&self) -> usize {
        let queue = self.queue.read().await;
        queue.len()
    }

    /// 队列是否为空
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// 清空队列
    pub async fn clear(&self) {
        let mut queue = self.queue.write().await;
        queue.clear();
    }
}

#[async_trait]
impl MessageStore for InMemoryQueue {
    async fn save(&self, msg: &Message) -> HsbResult<()> {
        self.enqueue(msg.clone()).await
    }

    async fn get(&self, id: &str) -> HsbResult<Option<Message>> {
        let queue = self.queue.read().await;
        Ok(queue
            .iter()
            .find(|m| m.message.id.to_string() == id)
            .map(|m| m.message.clone()))
    }

    async fn delete(&self, id: &str) -> HsbResult<()> {
        let mut queue = self.queue.write().await;
        queue.retain(|m| m.message.id.to_string() != id);
        Ok(())
    }

    async fn update_status(&self, id: &str, status: MessageStatus) -> HsbResult<()> {
        let mut queue = self.queue.write().await;
        if let Some(msg) = queue.iter_mut().find(|m| m.message.id.to_string() == id) {
            msg.meta.status = status;
            msg.meta.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    async fn pending_messages(&self, limit: usize) -> HsbResult<Vec<Message>> {
        let queue = self.queue.read().await;
        Ok(queue
            .iter()
            .filter(|m| m.meta.status == MessageStatus::Pending)
            .take(limit)
            .map(|m| m.message.clone())
            .collect())
    }
}

/// 优先级队列
pub struct PriorityQueue {
    high: InMemoryQueue,
    medium: InMemoryQueue,
    low: InMemoryQueue,
}

impl PriorityQueue {
    pub fn new(max_size_per_priority: usize) -> Self {
        Self {
            high: InMemoryQueue::new(max_size_per_priority),
            medium: InMemoryQueue::new(max_size_per_priority),
            low: InMemoryQueue::new(max_size_per_priority),
        }
    }

    /// 入队（根据优先级）
    pub async fn enqueue(&self, msg: Message, priority: Priority) -> HsbResult<()> {
        match priority {
            Priority::High => self.high.enqueue(msg).await,
            Priority::Medium => self.medium.enqueue(msg).await,
            Priority::Low => self.low.enqueue(msg).await,
        }
    }

    /// 出队（按优先级顺序）
    pub async fn dequeue(&self) -> HsbResult<Option<QueuedMessage>> {
        // 先检查高优先级队列
        if let Some(msg) = self.high.dequeue().await? {
            return Ok(Some(msg));
        }

        // 再检查中优先级队列
        if let Some(msg) = self.medium.dequeue().await? {
            return Ok(Some(msg));
        }

        // 最后检查低优先级队列
        self.low.dequeue().await
    }

    /// 总长度
    pub async fn total_len(&self) -> usize {
        self.high.len().await + self.medium.len().await + self.low.len().await
    }
}

/// 优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl From<i32> for Priority {
    fn from(value: i32) -> Self {
        if value >= 7 {
            Priority::High
        } else if value >= 4 {
            Priority::Medium
        } else {
            Priority::Low
        }
    }
}
