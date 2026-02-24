//! @AI: infrastructure/mcp — MCP adapter routing map
//! - standard_mcp_adapter -> StandardMcpAdapter: routes McpClientPort dispatches to
//!   registered ToolHandler implementations by tool name.
//!   Also exports the ToolHandler trait for concrete tool adapters to implement.
//! - stdio_mcp_client    -> StdioMcpClient: MCP wire protocol over stdin/stdout (JSON-RPC 2.0).
//! - sse_mcp_client      -> SseMcpClient: MCP wire protocol over HTTP+SSE.
//! - composite_mcp_client -> CompositeMcpClient: fans out to wire servers then compiled-in handlers.

mod composite_mcp_client;
mod sse_mcp_client;
mod standard_mcp_adapter;
mod stdio_mcp_client;

pub use composite_mcp_client::CompositeMcpClient;
pub use sse_mcp_client::SseMcpClient;
pub use standard_mcp_adapter::{StandardMcpAdapter, ToolHandler};
pub use stdio_mcp_client::StdioMcpClient;
