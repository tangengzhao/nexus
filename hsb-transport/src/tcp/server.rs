//! TCP 服务端

use hsb_common::{HsbError, HsbResult};
use hsb_core::{ConnectionContext, MessageHandler};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::{TcpServerConfig, mllp};

/// TCP 服务端
pub struct TcpServer {
    config: TcpServerConfig,
    handler: Arc<RwLock<Option<Arc<dyn MessageHandler>>>>,
    running: Arc<RwLock<bool>>,
}

impl TcpServer {
    pub fn new(config: TcpServerConfig) -> Self {
        Self {
            config,
            handler: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 设置消息处理器
    pub async fn set_handler(&self, handler: Arc<dyn MessageHandler>) {
        let mut h = self.handler.write().await;
        *h = Some(handler);
    }

    /// 启动服务
    pub async fn start(&self) -> HsbResult<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.bind_addr, self.config.bind_port)
            .parse()
            .map_err(|e| HsbError::ConfigError {
                message: format!("Invalid bind address: {}", e),
            })?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("Failed to bind: {}", e),
            })?;

        info!("TCP server listening on {}", addr);

        {
            let mut running = self.running.write().await;
            *running = true;
        }

        let handler = self.handler.clone();
        let config = self.config.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let handler = handler.clone();
                        let config = config.clone();

                        tokio::spawn(async move {
                            if let Err(e) =
                                handle_connection(stream, peer_addr, handler, config).await
                            {
                                error!("Connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止服务
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// 是否正在运行
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    handler: Arc<RwLock<Option<Arc<dyn MessageHandler>>>>,
    config: TcpServerConfig,
) -> HsbResult<()> {
    info!("New connection from {}", peer_addr);

    let local_addr = stream
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_default();
    let connection_id = ulid::Ulid::new().to_string();

    let mut codec = mllp::MllpCodec::new();
    let mut buffer = vec![0u8; config.buffer_size];

    loop {
        let n = match stream.read(&mut buffer).await {
            Ok(0) => {
                info!("Connection closed by {}", peer_addr);
                break;
            }
            Ok(n) => n,
            Err(e) => {
                warn!("Read error from {}: {}", peer_addr, e);
                break;
            }
        };

        // 解析消息
        let messages = if config.use_mllp {
            codec.input(&buffer[..n])
        } else {
            vec![buffer[..n].to_vec()]
        };

        for message_data in messages {
            let handler_guard = handler.read().await;

            if let Some(ref h) = *handler_guard {
                let context = ConnectionContext {
                    remote_addr: peer_addr.to_string(),
                    local_addr: local_addr.clone(),
                    connection_id: connection_id.clone(),
                    tls_info: None,
                };

                match h.handle(bytes::Bytes::from(message_data), context).await {
                    Ok(response) => {
                        let response_data = if config.use_mllp {
                            mllp::wrap_mllp(&response)
                        } else {
                            response.to_vec()
                        };

                        if let Err(e) = stream.write_all(&response_data).await {
                            error!("Write error to {}: {}", peer_addr, e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Handler error: {}", e);
                    }
                }
            } else {
                warn!("No handler configured");
            }
        }
    }

    Ok(())
}
