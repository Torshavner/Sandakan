use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{McpClientPort, McpError};
use crate::domain::{ToolCall, ToolResult};
use crate::infrastructure::mcp::StandardMcpAdapter;

/// Routes a `call_tool` request to the first wire client whose `call_tool`
/// succeeds, then falls back to the compiled-in `StandardMcpAdapter`.
///
/// Tool-not-found errors are not treated as failures — they cause the router to
/// try the next client. Any other error is surfaced immediately.
pub struct CompositeMcpClient {
    wire: Vec<Arc<dyn McpClientPort>>,
    local: StandardMcpAdapter,
}

impl CompositeMcpClient {
    pub fn new(wire: Vec<Arc<dyn McpClientPort>>, local: StandardMcpAdapter) -> Self {
        Self { wire, local }
    }
}

#[async_trait]
impl McpClientPort for CompositeMcpClient {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        for client in &self.wire {
            match client.call_tool(call).await {
                Ok(result) => return Ok(result),
                // Tool not registered on this server — try the next one.
                Err(McpError::ToolNotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        // Fall through to compiled-in handlers.
        self.local.call_tool(call).await
    }
}
