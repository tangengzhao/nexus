//! 消息分发器

use crate::engine::{EndpointRegistry, PendingReply, WebhookRegistry};
use crate::persistence::IdempotencyStore;
use crate::transport::{TransportRegistry, TransportRequest};
use crate::{DeliveryMode, MessageContext, ReplyMode, Route, RouteTarget};
use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::constants::http as http_headers;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::{DispatchResult, Dispatcher};

/// 默认分发器
pub struct DefaultDispatcher {
    transports: Arc<RwLock<TransportRegistry>>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    idempotency: Option<Arc<dyn IdempotencyStore>>,
    webhooks: Arc<WebhookRegistry>,
    config: DispatcherConfig,
}

/// 延时回复投递计划（来自路由的 Webhook 配置）
#[derive(Debug, Clone)]
struct DeferredReplyPlan {
    route_id: String,
    callback_url: String,
    ttl_secs: u64,
}

/// 延时回复转发结果
#[derive(Debug, Clone)]
pub struct ReplyDelivery {
    pub token: String,
    pub route_id: String,
    pub message_id: String,
    pub callback_url: String,
    /// 入口端回调接口返回的 HTTP 状态码
    pub status_code: u16,
    pub duration_ms: u64,
}

impl DefaultDispatcher {
    pub fn new(
        transports: Arc<RwLock<TransportRegistry>>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
    ) -> Self {
        Self {
            transports,
            endpoints,
            idempotency: None,
            webhooks: Arc::new(WebhookRegistry::new()),
            config: DispatcherConfig::default(),
        }
    }

    pub fn with_idempotency(
        transports: Arc<RwLock<TransportRegistry>>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
        idempotency: Arc<dyn IdempotencyStore>,
    ) -> Self {
        Self {
            transports,
            endpoints,
            idempotency: Some(idempotency),
            webhooks: Arc::new(WebhookRegistry::new()),
            config: DispatcherConfig::default(),
        }
    }

    pub fn with_config(
        transports: Arc<RwLock<TransportRegistry>>,
        endpoints: Arc<RwLock<EndpointRegistry>>,
        config: DispatcherConfig,
    ) -> Self {
        Self {
            transports,
            endpoints,
            idempotency: None,
            webhooks: Arc::new(WebhookRegistry::new()),
            config,
        }
    }

    /// 延时回复事务登记表
    pub fn webhooks(&self) -> Arc<WebhookRegistry> {
        self.webhooks.clone()
    }

    /// 转发出口端的延时回复：按令牌（或唯一关联 ID）找到事务，POST 到入口端回调 URL。
    ///
    /// 回调失败时事务会被放回登记表，出口端可以重试。
    pub async fn deliver_reply(
        &self,
        key: &str,
        body: Bytes,
        content_type: Option<&str>,
    ) -> HsbResult<ReplyDelivery> {
        let pending = self.webhooks.take(key).ok_or_else(|| HsbError::NotFound {
            entity: "PendingReply".to_string(),
            id: key.to_string(),
        })?;

        let start = Instant::now();
        let transports = self.transports.read().await;
        let Some(transport) = transports.get("http") else {
            self.webhooks.restore(pending);
            return Err(HsbError::ConfigError {
                message: "Transport not found: http".to_string(),
            });
        };

        let mut request = TransportRequest::new(&pending.callback_url, body)
            .with_timeout(Duration::from_secs(self.config.default_timeout_secs))
            .with_trace_id(&pending.trace_id)
            .with_header(http_headers::HEADER_CORRELATION_ID, &pending.correlation_id)
            .with_header(http_headers::HEADER_REPLY_TOKEN, &pending.token)
            .with_header(http_headers::HEADER_SOURCE_SYSTEM, &pending.target_endpoint)
            .with_header("X-HSB-Message-Id", &pending.message_id)
            .with_header("X-HSB-Route-Id", &pending.route_id);
        if let Some(content_type) = content_type {
            request = request.with_header("Content-Type", content_type);
        }

        let mut last_error = String::new();
        for attempt in 0..=self.config.default_retry_count {
            if attempt > 0 {
                tokio::time::sleep(self.calculate_retry_delay(attempt)).await;
            }
            match transport.send(request.clone()).await {
                Ok(response) if response.is_success() => {
                    info!(
                        "Webhook reply for route {} delivered to {}",
                        pending.route_id, pending.callback_url
                    );
                    return Ok(ReplyDelivery {
                        token: pending.token,
                        route_id: pending.route_id,
                        message_id: pending.message_id,
                        callback_url: pending.callback_url,
                        status_code: response.status_code,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                }
                Ok(response) => {
                    last_error = format!("callback returned HTTP {}", response.status_code);
                }
                Err(e) => last_error = e.to_string(),
            }
            warn!(
                "Webhook callback attempt {} to {} failed: {}",
                attempt + 1,
                pending.callback_url,
                last_error
            );
        }

        let service = format!("webhook callback {}: {}", pending.callback_url, last_error);
        self.webhooks.restore(pending);
        Err(HsbError::ServiceUnavailable { service })
    }

    fn resolve_transport_name(
        &self,
        protocol: ProtocolType,
        address: &str,
    ) -> HsbResult<&'static str> {
        match protocol {
            ProtocolType::Http
            | ProtocolType::Webhook
            | ProtocolType::FhirR5
            | ProtocolType::Soap
            | ProtocolType::OpenAi
            | ProtocolType::Mcp => Ok("http"),
            ProtocolType::Hl7V3 => {
                if address.starts_with("grpc://") {
                    Ok("grpc")
                } else if address.starts_with("kafka://") {
                    Ok("kafka")
                } else if address.starts_with("nats://") {
                    Ok("nats")
                } else if address.starts_with("amqp://") || address.starts_with("rabbitmq://") {
                    Ok("mq")
                } else if address.starts_with("tcp://") || address.starts_with("mllp://") {
                    Ok("tcp")
                } else {
                    Ok("http")
                }
            }
            ProtocolType::Hl7V2 | ProtocolType::TcpRaw | ProtocolType::Dicom => Ok("tcp"),
            ProtocolType::Grpc => Ok("grpc"),
            ProtocolType::MessageQueue => {
                if address.starts_with("kafka://") {
                    Ok("kafka")
                } else if address.starts_with("nats://") {
                    Ok("nats")
                } else {
                    Ok("mq")
                }
            }
            ProtocolType::Database => Err(HsbError::ProtocolNotSupported {
                protocol: "DATABASE".to_string(),
            }),
            ProtocolType::Custom => Err(HsbError::ProtocolNotSupported {
                protocol: "CUSTOM".to_string(),
            }),
        }
    }

    async fn dispatch_to_target(
        &self,
        ctx: &MessageContext,
        target: &RouteTarget,
        deferred: Option<&DeferredReplyPlan>,
    ) -> HsbResult<DispatchResult> {
        let start = Instant::now();
        let msg = ctx.message().await;

        let endpoint = {
            let endpoints = self.endpoints.read().await;
            endpoints
                .get(target.endpoint_id.as_str())
                .cloned()
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Endpoint".to_string(),
                    id: target.endpoint_id.to_string(),
                })?
        };

        let transport_name = self.resolve_transport_name(endpoint.protocol, &endpoint.address)?;
        let transports = self.transports.read().await;

        let transport = transports
            .get(transport_name)
            .ok_or_else(|| HsbError::ConfigError {
                message: format!("Transport not found: {}", transport_name),
            })?;

        // 构建请求体
        let body = if !msg.raw_payload.is_empty() {
            Bytes::from(msg.raw_payload.clone())
        } else if let Some(ref payload) = msg.payload {
            Bytes::from(serde_json::to_vec(payload).unwrap_or_default())
        } else {
            Bytes::new()
        };

        let endpoint_url = endpoint.address.clone();
        let mut request = TransportRequest::new(&endpoint_url, body)
            .with_timeout(Duration::from_secs(self.config.default_timeout_secs));

        request.metadata.target_system = Some(target.endpoint_id.clone());
        request.metadata.source_system = Some(msg.source_system.clone());
        request.metadata.message_id = Some(msg.id.to_string());

        // 添加追踪头
        request = request.with_trace_id(&ctx.trace_id.to_string());

        // 延时回复：发送前先登记事务，避免出口端极快回复时找不到事务
        let mut reply_token = None;
        if let Some(plan) = deferred {
            let token = ulid::Ulid::new().to_string();
            let correlation_id = msg
                .correlation_id
                .clone()
                .unwrap_or_else(|| msg.id.to_string());
            request = request
                .with_header(
                    http_headers::HEADER_REPLY_MODE,
                    ReplyMode::Deferred.as_str(),
                )
                .with_header(http_headers::HEADER_REPLY_TOKEN, &token)
                .with_header(http_headers::HEADER_CORRELATION_ID, &correlation_id);
            self.webhooks.register(
                PendingReply {
                    token: token.clone(),
                    correlation_id,
                    message_id: msg.id.to_string(),
                    trace_id: ctx.trace_id.to_string(),
                    route_id: plan.route_id.clone(),
                    target_endpoint: target.endpoint_id.to_string(),
                    callback_url: plan.callback_url.clone(),
                    created_at: chrono::Utc::now(),
                    expires_at: chrono::Utc::now(),
                },
                plan.ttl_secs,
            );
            reply_token = Some(token);
        }

        // 重试逻辑
        let max_retries = self.config.default_retry_count;
        let mut last_error = None;
        let mut retry_count = 0;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = self.calculate_retry_delay(attempt);
                tokio::time::sleep(delay).await;
                retry_count = attempt;
            }

            match transport.send(request.clone()).await {
                Ok(response) => {
                    let duration = start.elapsed();

                    if response.is_success() {
                        let mut result = DispatchResult::success(
                            &target.endpoint_id.to_string(),
                            &endpoint_url,
                            Some(response.body),
                            duration.as_millis() as u64,
                        );
                        if let (Some(token), Some(plan)) = (reply_token, deferred) {
                            result.reply_mode = ReplyMode::Deferred;
                            result.reply_token = Some(token);
                            result.callback_url = Some(plan.callback_url.clone());
                        }
                        return Ok(result);
                    } else {
                        last_error = Some(format!(
                            "HTTP {}: {:?}",
                            response.status_code, response.body
                        ));
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    warn!("Dispatch attempt {} failed: {}", attempt + 1, e);
                }
            }
        }

        if let Some(token) = &reply_token {
            self.webhooks.remove(token);
        }

        let duration = start.elapsed();
        Ok(DispatchResult::failure(
            &target.endpoint_id.to_string(),
            &endpoint_url,
            &last_error.unwrap_or_else(|| "Unknown error".to_string()),
            duration.as_millis() as u64,
            retry_count,
        ))
    }

    fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.config.retry_base_delay_ms;
        let delay = base_delay * (2u64.pow(attempt.min(10)));
        Duration::from_millis(delay.min(self.config.retry_max_delay_ms))
    }

    async fn dispatch_exactly_once(
        &self,
        ctx: &MessageContext,
        route: &Route,
        target: &RouteTarget,
        deferred: Option<&DeferredReplyPlan>,
    ) -> HsbResult<DispatchResult> {
        let Some(idempotency) = &self.idempotency else {
            return self.dispatch_to_target(ctx, target, deferred).await;
        };

        let idempotency_key = {
            let msg = ctx.message().await;
            let unique_key = msg
                .correlation_id
                .clone()
                .unwrap_or_else(|| msg.id.to_string());
            format!("{}:{}:{}", route.id, target.endpoint_id, unique_key)
        };

        let is_new = idempotency
            .check_and_mark(&idempotency_key, 24 * 60 * 60)
            .await?;
        if !is_new {
            return Ok(DispatchResult::success(
                &route.id.to_string(),
                target.endpoint_id.as_str(),
                None,
                0,
            ));
        }

        let result = self.dispatch_to_target(ctx, target, deferred).await?;
        if result.success {
            Ok(result)
        } else {
            idempotency.clear_mark(&idempotency_key).await?;
            Ok(result)
        }
    }
}

#[async_trait]
impl Dispatcher for DefaultDispatcher {
    async fn dispatch(&self, ctx: &MessageContext, route: &Route) -> HsbResult<DispatchResult> {
        // 先按分拣分支选出口：首个命中的分支生效，否则走默认出口
        let msg = ctx.message().await.clone();
        let (branch, targets) = route.resolve_targets(&msg);
        let branch = branch.map(str::to_string);

        if targets.is_empty() {
            return if route.branches.is_empty() {
                Err(HsbError::ConfigError {
                    message: format!("Route {} has no targets", route.id),
                })
            } else {
                Ok(DispatchResult::failure(
                    &route.id.to_string(),
                    "no_branch_matched",
                    "未命中任何分拣分支，且未配置默认出口端点",
                    0,
                    0,
                ))
            };
        }

        let mut result = self.dispatch_targets(ctx, route, targets).await?;
        result.branch = branch;
        Ok(result)
    }

    async fn dispatch_batch(
        &self,
        ctx: &MessageContext,
        routes: &[Route],
    ) -> HsbResult<Vec<DispatchResult>> {
        let mut results = Vec::new();

        for route in routes {
            let msg = ctx.message().await.clone();
            let (_, targets) = route.resolve_targets(&msg);
            for target in targets {
                let result = self.dispatch_to_target(ctx, target, None).await?;
                results.push(result);
            }
        }

        Ok(results)
    }
}

impl DefaultDispatcher {
    async fn dispatch_targets(
        &self,
        ctx: &MessageContext,
        route: &Route,
        targets: &[RouteTarget],
    ) -> HsbResult<DispatchResult> {
        let deferred = match route.options.reply_mode {
            ReplyMode::Realtime => None,
            ReplyMode::Deferred => {
                route.options.validate_reply()?;
                Some(DeferredReplyPlan {
                    route_id: route.id.to_string(),
                    callback_url: route
                        .options
                        .callback_url
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or_default()
                        .to_string(),
                    ttl_secs: route.options.reply_timeout_secs,
                })
            }
        };
        let deferred = deferred.as_ref();

        // 根据投递模式选择目标
        match route.options.delivery_mode {
            DeliveryMode::AtMostOnce => {
                // 最多一次：只发送到第一个目标，不重试
                self.dispatch_to_target(ctx, &targets[0], deferred).await
            }
            DeliveryMode::AtLeastOnce => {
                // 至少一次：发送到第一个目标，失败时尝试备用目标
                for target in targets {
                    let result = self.dispatch_to_target(ctx, target, deferred).await?;
                    if result.success {
                        return Ok(result);
                    }
                }
                // 所有目标都失败
                Ok(DispatchResult::failure(
                    &route.id.to_string(),
                    "all_targets",
                    "All targets failed",
                    0,
                    0,
                ))
            }
            DeliveryMode::ExactlyOnce => {
                let primary = targets.iter().find(|t| t.primary).unwrap_or(&targets[0]);
                self.dispatch_exactly_once(ctx, route, primary, deferred)
                    .await
            }
        }
    }
}

/// 分发器配置
#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    /// 默认超时（秒）
    pub default_timeout_secs: u64,
    /// 默认重试次数
    pub default_retry_count: u32,
    /// 重试基础延迟（毫秒）
    pub retry_base_delay_ms: u64,
    /// 重试最大延迟（毫秒）
    pub retry_max_delay_ms: u64,
    /// 最大并发分发数
    pub max_concurrent_dispatches: usize,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: 30,
            default_retry_count: 3,
            retry_base_delay_ms: 100,
            retry_max_delay_ms: 30000,
            max_concurrent_dispatches: 100,
        }
    }
}
