//! MCP（Model Context Protocol）支持
//!
//! - [`McpClient`]：总线作为客户端，调用下游 MCP Server（Streamable HTTP）
//! - [`McpGateway`]：总线作为 MCP Server，对智能体暴露聚合后的工具

mod client;
mod gateway;

pub use client::{
    DEFAULT_MCP_TIMEOUT_SECS, McpClient, McpError, SUPPORTED_PROTOCOL_VERSIONS, codes,
};
pub use gateway::{
    McpAuthError, McpCallRecord, McpCaller, McpClientConfig, McpEndpointStatus, McpGateway,
    McpGatewayConfig, McpToolCatalog, McpToolInfo, TOOL_NAME_SEPARATOR, endpoint_prefix,
    sanitize_prefix,
};
