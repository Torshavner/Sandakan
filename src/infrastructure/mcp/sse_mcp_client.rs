use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::ports::{McpClientPort, McpError, ToolSchema};
use crate::domain::{ToolCall, ToolCallId, ToolName, ToolResult};

// ─── JSON-RPC 2.0 wire types ─────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: Value,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResponse {
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    message: String,
}

// ─── SseMcpClient ────────────────────────────────────────────────────────────

/// MCP client that communicates with a remote MCP server over HTTP+SSE.
///
/// Each `call_tool` invocation:
///  1. POSTs a JSON-RPC request to `{endpoint}/message`
///  2. Reads the SSE stream response (one `data:` event with the JSON-RPC reply)
///
/// The client is stateless per-call — no persistent connection is kept.
/// `tool_schemas` is populated once at construction via `tools/list`.
pub struct SseMcpClient {
    client: Client,
    /// Base URL of the MCP server, e.g. `http://localhost:3000`.
    endpoint: String,
    next_id: AtomicU64,
    /// Tool schemas advertised by the server, populated at startup.
    pub tool_schemas: Vec<ToolSchema>,
}

impl SseMcpClient {
    /// Connect to the MCP server at `endpoint`, perform `initialize` + `tools/list`.
    pub async fn new(endpoint: impl Into<String>) -> Result<Self, McpError> {
        let client = Client::new();
        let endpoint = endpoint.into();
        let next_id = AtomicU64::new(1);

        let proto_client = Self {
            client,
            endpoint,
            next_id,
            tool_schemas: Vec::new(),
        };

        proto_client.initialize().await?;
        let schemas = proto_client.list_tools().await?;

        Ok(Self {
            tool_schemas: schemas,
            ..proto_client
        })
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn send_request(&self, method: &'static str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id();
        let body = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let url = format!("{}/message", self.endpoint);

        let response = self
            .client
            .post(&url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| McpError::Transport(format!("POST {url}: {e}")))?;

        if !response.status().is_success() {
            return Err(McpError::Transport(format!(
                "MCP server returned HTTP {}",
                response.status()
            )));
        }

        // The SSE stream carries exactly one `data:` event for a request/response
        // cycle. We read the raw body and extract the first data line.
        let text = response
            .text()
            .await
            .map_err(|e| McpError::Transport(format!("reading SSE body: {e}")))?;

        let json_str = parse_first_sse_data(&text)?;

        let rpc: JsonRpcResponse = serde_json::from_str(json_str)
            .map_err(|e| McpError::Serialization(format!("parsing JSON-RPC response: {e}")))?;

        if let Some(err) = rpc.error {
            return Err(McpError::Protocol(err.message));
        }

        rpc.result
            .ok_or_else(|| McpError::Protocol("response missing 'result'".to_string()))
    }

    async fn initialize(&self) -> Result<(), McpError> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "sandakan", "version": "0.1.0" }
        });
        self.send_request("initialize", params).await?;
        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<ToolSchema>, McpError> {
        let result = self
            .send_request("tools/list", Value::Object(Default::default()))
            .await?;

        let tools = result["tools"]
            .as_array()
            .ok_or_else(|| McpError::Protocol("tools/list missing 'tools' array".to_string()))?;

        let schemas = tools
            .iter()
            .filter_map(|t| {
                let name = t["name"].as_str()?.to_string();
                let description = t["description"].as_str().unwrap_or("").to_string();
                let parameters = t["inputSchema"].clone();
                Some(ToolSchema {
                    name,
                    description,
                    parameters,
                })
            })
            .collect();

        Ok(schemas)
    }
}

#[async_trait]
impl McpClientPort for SseMcpClient {
    async fn call_tool(&self, call: &ToolCall) -> Result<ToolResult, McpError> {
        let params = serde_json::json!({
            "name": call.name.as_str(),
            "arguments": call.arguments,
        });

        let result = self.send_request("tools/call", params).await?;

        let content = extract_text_content(&result)?;

        Ok(ToolResult {
            tool_call_id: ToolCallId::new(call.id.as_str()),
            tool_name: ToolName::new(call.name.as_str()),
            content,
        })
    }
}

// ─── SSE parsing helper ───────────────────────────────────────────────────────

/// Extract the payload of the first `data:` line from a raw SSE body.
fn parse_first_sse_data(body: &str) -> Result<&str, McpError> {
    body.lines()
        .find(|line| line.starts_with("data:"))
        .map(|line| line["data:".len()..].trim())
        .ok_or_else(|| McpError::Protocol("no data: event in SSE response".to_string()))
}

// ─── Content extraction helper ────────────────────────────────────────────────

/// MCP `tools/call` returns `{ "content": [{ "type": "text", "text": "..." }] }`.
fn extract_text_content(result: &Value) -> Result<String, McpError> {
    let items = result["content"]
        .as_array()
        .ok_or_else(|| McpError::Protocol("tools/call missing 'content' array".to_string()))?;

    let text = items
        .iter()
        .filter(|item| item["type"].as_str() == Some("text"))
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");

    Ok(text)
}
