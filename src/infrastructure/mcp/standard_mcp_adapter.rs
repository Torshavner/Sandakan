use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::application::ports::{McpClientPort, McpError};
use crate::domain::{ToolCall, ToolResult};

/// A concrete tool executor that can be registered with `StandardMcpAdapter`.
///
/// Each tool (e.g. `WebSearchAdapter`) implements this trait, allowing the adapter
/// to route `McpClientPort::call_tool()` dispatches by tool name.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn tool_name(&self) -> &str;
    async fn execute(&self, arguments: &serde_json::Value) -> Result<String, McpError>;
}

/// Routes `McpClientPort` dispatches to registered `ToolHandler` implementations.
///
/// This is the MCP stub: it holds a map of handlers keyed by tool name.
/// A future story (US-022) can replace this with a real MCP stdio/SSE transport
/// while keeping the `McpClientPort` interface unchanged.
pub struct StandardMcpAdapter {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl StandardMcpAdapter {
    pub fn new(handlers: Vec<Arc<dyn ToolHandler>>) -> Self {
        let map = handlers
            .into_iter()
            .map(|h| (h.tool_name().to_string(), h))
            .collect();
        Self { handlers: map }
    }
}

#[async_trait]
impl McpClientPort for StandardMcpAdapter {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        let handler = self
            .handlers
            .get(call.name.as_str())
            .ok_or_else(|| McpError::ToolNotFound(call.name.to_string()))?;

        let content = handler.execute(&call.arguments).await?;

        Ok(ToolResult {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content,
        })
    }
}
