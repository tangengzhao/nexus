//! gRPC 服务器

use hsb_common::{HsbError, HsbResult};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::transport::Server;
use tracing::info;

use super::GrpcServerConfig;

/// gRPC 服务器
pub struct GrpcServer {
    config: GrpcServerConfig,
    running: Arc<RwLock<bool>>,
}

impl GrpcServer {
    pub fn new(config: GrpcServerConfig) -> Self {
        Self {
            config,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动服务器
    pub async fn start<S>(&self, service: S) -> HsbResult<()>
    where
        S: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<tonic::body::BoxBody>,
                Error = std::convert::Infallible,
            > + tonic::server::NamedService
            + Clone
            + Send
            + 'static,
        S::Future: Send + 'static,
    {
        let addr: SocketAddr =
            self.config
                .address()
                .parse()
                .map_err(|e| HsbError::ConfigError {
                    message: format!("Invalid address: {}", e),
                })?;

        {
            let mut running = self.running.write().await;
            *running = true;
        }

        info!("Starting gRPC server on {}", addr);

        let mut builder =
            Server::builder().max_concurrent_streams(Some(self.config.max_concurrent_streams));

        builder
            .add_service(service)
            .serve(addr)
            .await
            .map_err(|e| HsbError::InternalError {
                message: format!("gRPC server error: {}", e),
            })?;

        Ok(())
    }

    /// 停止服务器
    pub async fn stop(&self) -> HsbResult<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("gRPC server stopped");
        Ok(())
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// 健康检查服务
pub mod health {
    use tonic::{Request, Response, Status};

    /// 健康检查请求
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct HealthCheckRequest {
        #[prost(string, tag = "1")]
        pub service: String,
    }

    /// 健康检查响应
    #[derive(Clone, PartialEq, prost::Message)]
    pub struct HealthCheckResponse {
        #[prost(enumeration = "ServingStatus", tag = "1")]
        pub status: i32,
    }

    /// 服务状态
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, prost::Enumeration)]
    #[repr(i32)]
    pub enum ServingStatus {
        Unknown = 0,
        Serving = 1,
        NotServing = 2,
        ServiceUnknown = 3,
    }

    /// 健康检查服务实现
    #[derive(Debug, Default)]
    pub struct HealthService;

    #[tonic::async_trait]
    impl Health for HealthService {
        async fn check(
            &self,
            _request: Request<HealthCheckRequest>,
        ) -> Result<Response<HealthCheckResponse>, Status> {
            Ok(Response::new(HealthCheckResponse {
                status: ServingStatus::Serving as i32,
            }))
        }
    }

    /// 健康检查 trait
    #[tonic::async_trait]
    pub trait Health: Send + Sync + 'static {
        async fn check(
            &self,
            request: Request<HealthCheckRequest>,
        ) -> Result<Response<HealthCheckResponse>, Status>;
    }
}
