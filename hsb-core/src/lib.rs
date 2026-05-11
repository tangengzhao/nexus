//! HSB Core - 医院服务总线核心模块
//!
//! 本模块定义核心消息模型、路由规则、转换器和工作流，
//! 以及协议适配器/传输层的基础 trait、路由引擎、持久化层、可靠性层和集群通信。

// --- Core types ---
pub mod catalog;
pub mod context;
pub mod endpoint;
pub mod message;
pub mod route;
pub mod transformer;
pub mod workflow;

// --- Base trait definitions (consumed by hsb-adapter / hsb-transport crates) ---
pub mod adapter;
pub mod transport;

// --- Sub-systems ---
pub mod cluster;
pub mod engine;
pub mod persistence;
pub mod reliability;

// --- Re-exports: core types ---
pub use catalog::{IntegrationSystem, Organization};
pub use context::{MessageContext, ProcessingRecord, ProcessingStage};
pub use endpoint::{
    AuthConfig, ConnectionConfig, Endpoint, EndpointConfig, EndpointLifecycleStatus,
    EndpointRuntimeStatus, EndpointSecurity, EndpointVersionRecord,
};
pub use message::{Message, MessageBuilder, MessageMeta, MessageMetadata};
pub use route::{
    DeliveryMode, MatchOperator, MatchRule, MatchSource, Route, RouteBuilder, RouteOptions,
    RouteTarget, SourceMatch,
};
pub use transformer::{TransformContext, Transformer, TransformerChain};
pub use workflow::{
    CompensationPolicy, InMemoryWorkflowExecutor, StepExecution, StepExecutionStatus, Workflow,
    WorkflowContext, WorkflowInstance, WorkflowStatus, WorkflowStep, WorkflowStepHandler,
};

// --- Re-exports: adapter base ---
pub use adapter::{
    AdapterFactory, AdapterRegistry, ErrorSeverity, ParseOptions, ProtocolAdapter,
    SerializeOptions, ValidationError, ValidationResult, ValidationWarning,
};

// --- Re-exports: transport base ---
pub use transport::{
    ConnectableTransport, ConnectionContext, ConnectionPoolConfig, HealthStatus,
    ListenableTransport, MessageHandler, RequestMetadata, ResponseMetadata,
    RetryConfig as TransportRetryConfig, TlsInfo, Transport, TransportRegistry, TransportRequest,
    TransportResponse, TransportStats, TransportType,
};
