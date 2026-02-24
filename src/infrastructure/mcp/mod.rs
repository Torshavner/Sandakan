//! @AI: infrastructure/mcp — MCP adapter routing map
//! - standard_mcp_adapter -> StandardMcpAdapter: routes McpClientPort dispatches to
//!   registered ToolHandler implementations by tool name.
//!   Also exports the ToolHandler trait for concrete tool adapters to implement.

mod standard_mcp_adapter;

pub use standard_mcp_adapter::{StandardMcpAdapter, ToolHandler};
