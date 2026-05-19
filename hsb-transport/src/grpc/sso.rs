//! rust-sso 集成
//!
//! 提供与 rust-sso 系统的 gRPC 集成，用于认证和授权。

use hsb_common::{HsbError, HsbResult};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::transport::Channel;
use tracing::info;

/// SSO 客户端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsoClientConfig {
    /// SSO gRPC 端点
    pub endpoint: String,
    /// 客户端 ID
    pub client_id: String,
    /// 客户端密钥
    pub client_secret: String,
    /// 连接超时（秒）
    pub connect_timeout_secs: u64,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// 令牌缓存时间（秒）
    pub token_cache_secs: u64,
    /// 使用 TLS
    pub use_tls: bool,
}

impl Default for SsoClientConfig {
    fn default() -> Self {
        Self {
            endpoint: env_string("RUST_SSO_GRPC_ENDPOINT", "http://rust-sso:50051"),
            client_id: String::new(),
            client_secret: String::new(),
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            token_cache_secs: 300,
            use_tls: false,
        }
    }
}

fn env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// SSO 客户端
pub struct SsoClient {
    config: SsoClientConfig,
    channel: Arc<RwLock<Option<Channel>>>,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

/// 缓存的令牌
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl SsoClient {
    pub fn new(config: SsoClientConfig) -> Self {
        Self {
            config,
            channel: Arc::new(RwLock::new(None)),
            token_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// 连接到 SSO 服务
    pub async fn connect(&self) -> HsbResult<()> {
        let endpoint = tonic::transport::Endpoint::from_shared(self.config.endpoint.clone())
            .map_err(|e| HsbError::ConfigError {
                message: format!("Invalid SSO endpoint: {}", e),
            })?
            .connect_timeout(Duration::from_secs(self.config.connect_timeout_secs))
            .timeout(Duration::from_secs(self.config.request_timeout_secs));

        let channel = endpoint
            .connect()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.config.endpoint.clone(),
                message: e.to_string(),
            })?;

        let mut ch = self.channel.write().await;
        *ch = Some(channel);

        info!("Connected to SSO service at {}", self.config.endpoint);
        Ok(())
    }

    /// 验证令牌
    pub async fn validate_token(&self, token: &str) -> HsbResult<TokenValidation> {
        // 简化实现：假设令牌验证成功
        // 实际实现需要调用 SSO 的 gRPC 服务

        if token.is_empty() {
            return Err(HsbError::AuthenticationError {
                message: "Token is empty".to_string(),
            });
        }

        // TODO: 调用实际的 SSO gRPC 服务验证令牌
        Ok(TokenValidation {
            valid: true,
            user_id: Some("user123".to_string()),
            username: Some("test_user".to_string()),
            roles: vec!["admin".to_string()],
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
        })
    }

    /// 获取服务令牌（用于服务间调用）
    pub async fn get_service_token(&self) -> HsbResult<String> {
        // 检查缓存
        {
            let cache = self.token_cache.read().await;
            if let Some(ref cached) = *cache {
                if cached.expires_at > chrono::Utc::now() {
                    return Ok(cached.access_token.clone());
                }
            }
        }

        // 请求新令牌
        let token = self.request_service_token().await?;

        // 更新缓存
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(CachedToken {
                access_token: token.clone(),
                expires_at: chrono::Utc::now()
                    + chrono::Duration::seconds(self.config.token_cache_secs as i64),
            });
        }

        Ok(token)
    }

    async fn request_service_token(&self) -> HsbResult<String> {
        // TODO: 调用 SSO 服务获取服务令牌
        // 使用 client_credentials 授权类型

        info!("Requesting new service token from SSO");

        // 简化实现
        Ok(format!("service_token_{}", ulid::Ulid::new()))
    }

    /// 检查权限
    pub async fn check_permission(
        &self,
        token: &str,
        _resource: &str,
        _action: &str,
    ) -> HsbResult<bool> {
        // TODO: 调用 SSO 服务检查权限

        let validation = self.validate_token(token).await?;

        if !validation.valid {
            return Ok(false);
        }

        // 简化：admin 角色拥有所有权限
        Ok(validation.roles.contains(&"admin".to_string()))
    }
}

/// 令牌验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidation {
    /// 是否有效
    pub valid: bool,
    /// 用户 ID
    pub user_id: Option<String>,
    /// 用户名
    pub username: Option<String>,
    /// 角色列表
    pub roles: Vec<String>,
    /// 过期时间
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// SSO 认证中间件
pub struct SsoAuthMiddleware {
    sso_client: Arc<SsoClient>,
}

impl SsoAuthMiddleware {
    pub fn new(sso_client: Arc<SsoClient>) -> Self {
        Self { sso_client }
    }

    /// 从请求中提取并验证令牌
    pub async fn authenticate(&self, authorization: Option<&str>) -> HsbResult<TokenValidation> {
        let token = authorization
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| HsbError::AuthenticationError {
                message: "Missing or invalid Authorization header".to_string(),
            })?;

        self.sso_client.validate_token(token).await
    }

    /// 检查资源权限
    pub async fn authorize(
        &self,
        authorization: Option<&str>,
        resource: &str,
        action: &str,
    ) -> HsbResult<()> {
        let token = authorization
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| HsbError::AuthenticationError {
                message: "Missing or invalid Authorization header".to_string(),
            })?;

        let has_permission = self
            .sso_client
            .check_permission(token, resource, action)
            .await?;

        if !has_permission {
            return Err(HsbError::AuthorizationError {
                message: format!("Permission denied for {}:{}", resource, action),
            });
        }

        Ok(())
    }
}

/// gRPC 服务认证拦截器
#[allow(dead_code)]
pub struct AuthInterceptor {
    sso_client: Arc<SsoClient>,
}

impl AuthInterceptor {
    pub fn new(sso_client: Arc<SsoClient>) -> Self {
        Self { sso_client }
    }
}

impl tonic::service::Interceptor for AuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        // 从元数据中获取令牌
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "));

        match token {
            Some(_) => Ok(request),
            None => Err(tonic::Status::unauthenticated(
                "Missing authorization token",
            )),
        }
    }
}
