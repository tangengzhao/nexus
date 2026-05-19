//! 消息分发器

use crate::engine::EndpointRegistry;
use crate::persistence::IdempotencyStore;
use crate::transport::{TransportRegistry, TransportRequest};
use crate::{DeliveryMode, MessageContext, Route, RouteTarget};
use async_trait::async_trait;
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

use super::{DispatchResult, Dispatcher};

/// 默认分发器
pub struct DefaultDispatcher {
    transports: Arc<RwLock<TransportRegistry>>,
    endpoints: Arc<RwLock<EndpointRegistry>>,
    idempotency: Option<Arc<dyn IdempotencyStore>>,
    config: DispatcherConfig,
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
            config,
        }
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
            | ProtocolType::OpenAi => Ok("http"),
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
                        return Ok(DispatchResult::success(
                            &target.endpoint_id.to_string(),
                            &endpoint_url,
                            Some(response.body),
                            duration.as_millis() as u64,
                        ));
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
    ) -> HsbResult<DispatchResult> {
        let Some(idempotency) = &self.idempotency else {
            return self.dispatch_to_target(ctx, target).await;
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

        let result = self.dispatch_to_target(ctx, target).await?;
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
        if route.targets.is_empty() {
            return Err(HsbError::ConfigError {
                message: format!("Route {} has no targets", route.id),
            });
        }

        // 根据投递模式选择目标
        match route.options.delivery_mode {
            DeliveryMode::AtMostOnce => {
                // 最多一次：只发送到第一个目标，不重试
                self.dispatch_to_target(ctx, &route.targets[0]).await
            }
            DeliveryMode::AtLeastOnce => {
                // 至少一次：发送到第一个目标，失败时尝试备用目标
                for target in &route.targets {
                    let result = self.dispatch_to_target(ctx, target).await?;
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
                if let Some(primary) = route.primary_target() {
                    self.dispatch_exactly_once(ctx, route, primary).await
                } else {
                    self.dispatch_exactly_once(ctx, route, &route.targets[0])
                        .await
                }
            }
        }
    }

    async fn dispatch_batch(
        &self,
        ctx: &MessageContext,
        routes: &[Route],
    ) -> HsbResult<Vec<DispatchResult>> {
        let mut results = Vec::new();

        for route in routes {
            for target in &route.targets {
                let result = self.dispatch_to_target(ctx, target).await?;
                results.push(result);
            }
        }

        Ok(results)
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
