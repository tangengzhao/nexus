//! HSB 传输层
//!
//! 所有传输协议的统一入口。基础 trait 定义在 `hsb-core::transport` 中，
//! 本 crate 提供 HTTP、TCP/MLLP、MQ、NATS/JetStream、gRPC 五种传输的具体实现。

pub mod grpc;
pub mod http;
pub mod kafka;
pub mod mq;
pub mod nats;
pub mod tcp;

// Re-export commonly used types
pub use grpc::{GrpcTransport, GrpcTransportConfig, SsoClient, SsoClientConfig};
pub use http::{HttpTransport, HttpTransportConfig};
pub use kafka::{KafkaConsumerConfig, KafkaMessage, KafkaTransport, KafkaTransportConfig};
pub use mq::{MqTransport, MqTransportConfig};
pub use nats::{JetStreamConfig, NatsTransport, NatsTransportConfig};
pub use tcp::{TcpMllpTransport, TcpTransportConfig};
