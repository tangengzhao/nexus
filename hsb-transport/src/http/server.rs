//! HTTP 服务端

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use hsb_core::MessageHandler;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// HTTP 服务配置
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    /// 绑定地址
    pub bind_addr: String,
    /// 绑定端口
    pub bind_port: u16,
    /// 最大请求体大小（字节）
    pub max_body_size: usize,
    /// 是否启用 TLS
    pub use_tls: bool,
    /// TLS 证书路径
    pub tls_cert_path: Option<String>,
    /// TLS 密钥路径
    pub tls_key_path: Option<String>,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            bind_port: 8080,
            max_body_size: 10 * 1024 * 1024, // 10MB
            use_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

/// HTTP 服务端状态
struct ServerState {
    handler: Option<Arc<dyn MessageHandler>>,
}

/// HTTP 服务端
pub struct HttpServer {
    config: HttpServerConfig,
    state: Arc<RwLock<ServerState>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl HttpServer {
    pub fn new(config: HttpServerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ServerState { handler: None })),
            shutdown_tx: None,
        }
    }

    /// 设置消息处理器
    pub async fn set_handler(&self, handler: Arc<dyn MessageHandler>) {
        let mut state = self.state.write().await;
        state.handler = Some(handler);
    }

    /// 启动服务
    pub async fn start(&mut self) -> HsbResult<()> {
        let state = self.state.clone();

        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/message", post(message_handler))
            .layer(middleware::from_fn(logging_middleware))
            .with_state(state);

        let addr: SocketAddr = format!("{}:{}", self.config.bind_addr, self.config.bind_port)
            .parse()
            .map_err(|e| HsbError::ConfigError {
                message: format!("Invalid bind address: {}", e),
            })?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        info!("Starting HTTP server on {}", addr);

        let listener =
            tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|e| HsbError::TransportError {
                    message: format!("Failed to bind: {}", e),
                })?;

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        Ok(())
    }

    /// 停止服务
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// 健康检查处理器
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// 消息处理器
async fn message_handler(
    State(state): State<Arc<RwLock<ServerState>>>,
    body: Bytes,
) -> impl IntoResponse {
    let state = state.read().await;

    match &state.handler {
        Some(handler) => {
            let context = hsb_core::ConnectionContext {
                remote_addr: "unknown".to_string(),
                local_addr: "unknown".to_string(),
                connection_id: ulid::Ulid::new().to_string(),
                tls_info: None,
            };

            match handler.handle(body, context).await {
                Ok(response) => (StatusCode::OK, response).into_response(),
                Err(e) => {
                    error!("Handler error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e)).into_response()
                }
            }
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, "No handler configured").into_response(),
    }
}

/// 日志中间件
async fn logging_middleware(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();

    info!(
        "{} {} - {} ({:?})",
        method,
        uri,
        response.status(),
        duration
    );

    response
}
