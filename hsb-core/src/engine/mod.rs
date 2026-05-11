//! HSB 路由和处理引擎
//!
//! 核心路由引擎，负责消息的路由匹配、转换和分发。

mod dispatcher;
mod pipeline;
mod registry;
mod router;

pub use dispatcher::*;
pub use pipeline::*;
pub use registry::*;
pub use router::*;

use crate::{Message, MessageContext, Route};
use async_trait::async_trait;
use hsb_common::HsbResult;

/// 消息处理器 Trait
#[async_trait]
pub trait MessageProcessor: Send + Sync {
    /// 处理消息
    async fn process(&self, ctx: &mut MessageContext) -> HsbResult<()>;

    /// 处理器名称
    fn name(&self) -> &str;

    /// 处理器优先级（数值越小优先级越高）
    fn priority(&self) -> i32 {
        0
    }
}

/// 路由器 Trait
#[async_trait]
pub trait Router: Send + Sync {
    /// 查找匹配的路由
    async fn find_routes(&self, msg: &Message) -> HsbResult<Vec<Route>>;

    /// 添加路由
    async fn add_route(&self, route: Route) -> HsbResult<()>;

    /// 移除路由
    async fn remove_route(&self, route_id: &str) -> HsbResult<()>;

    /// 获取所有路由
    async fn list_routes(&self) -> HsbResult<Vec<Route>>;
}

/// 分发器 Trait
#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// 分发消息到目标系统
    async fn dispatch(&self, ctx: &MessageContext, route: &Route) -> HsbResult<DispatchResult>;

    /// 批量分发
    async fn dispatch_batch(
        &self,
        ctx: &MessageContext,
        routes: &[Route],
    ) -> HsbResult<Vec<DispatchResult>>;
}

/// 分发结果
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// 路由 ID
    pub route_id: String,
    /// 目标系统
    pub target_system: String,
    /// 是否成功
    pub success: bool,
    /// 响应数据
    pub response: Option<bytes::Bytes>,
    /// 错误信息
    pub error: Option<String>,
    /// 耗时（毫秒）
    pub duration_ms: u64,
    /// 重试次数
    pub retry_count: u32,
}

impl DispatchResult {
    pub fn success(
        route_id: &str,
        target: &str,
        response: Option<bytes::Bytes>,
        duration_ms: u64,
    ) -> Self {
        Self {
            route_id: route_id.to_string(),
            target_system: target.to_string(),
            success: true,
            response,
            error: None,
            duration_ms,
            retry_count: 0,
        }
    }

    pub fn failure(
        route_id: &str,
        target: &str,
        error: &str,
        duration_ms: u64,
        retry_count: u32,
    ) -> Self {
        Self {
            route_id: route_id.to_string(),
            target_system: target.to_string(),
            success: false,
            response: None,
            error: Some(error.to_string()),
            duration_ms,
            retry_count,
        }
    }
}

/// 引擎配置
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// 最大并发处理数
    pub max_concurrency: usize,
    /// 处理超时（秒）
    pub processing_timeout_secs: u64,
    /// 是否启用消息追踪
    pub enable_tracing: bool,
    /// 是否启用指标收集
    pub enable_metrics: bool,
    /// 默认重试次数
    pub default_retry_count: u32,
    /// 是否启用 Dead Letter Queue
    pub enable_dlq: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 100,
            processing_timeout_secs: 30,
            enable_tracing: true,
            enable_metrics: true,
            default_retry_count: 3,
            enable_dlq: true,
        }
    }
}
