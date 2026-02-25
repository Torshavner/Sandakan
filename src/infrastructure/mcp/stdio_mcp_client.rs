// @AI-BYPASS-LENGTH: JSON-RPC 2.0 framing over stdio requires managing the full
// request/response correlation loop (pending map, reader task, shutdown) in one place.
// Splitting would break the ownership invariants of ChildStdin/ChildStdout.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, oneshot};

use crate::application::ports::{McpClientPort, McpError};
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
    id: Option<u64>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug)]
struct JsonRpcError {
    message: String,
}

// ─── Shared pending-request map ───────────────────────────────────────────────

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>;

// ─── StdioMcpClient ──────────────────────────────────────────────────────────

/// MCP client that communicates with a child process over stdin/stdout using
/// JSON-RPC 2.0 framing (one JSON object per line, no Content-Length header).
///
/// Lifecycle: `new()` spawns the process and calls `initialize` + `tools/list`.
/// On drop the child is killed; callers should not retry after `ServerExited`.
pub struct StdioMcpClient {
    // Stored so the child is not orphaned when the client is dropped.
    _child: Child,
    stdin: Mutex<ChildStdin>,
    pending: PendingMap,
    next_id: Mutex<u64>,
    /// Tool schemas advertised by the server, populated at startup.
    pub tool_schemas: Vec<crate::application::ports::ToolSchema>,
}

impl StdioMcpClient {
    /// Spawn `command args` and perform the MCP handshake:
    /// `initialize` → `initialized` notification → `tools/list`.
    ///
    /// Returns the fully-ready client together with the tool schemas from
    /// `tools/list` so the caller can populate the `StaticToolRegistry`.
    pub async fn new(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            // Route the child's stderr to our stderr so tracing sees it.
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| McpError::Transport(format!("failed to spawn '{command}': {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("failed to acquire child stdin".to_string()))?;
        let stdout: ChildStdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("failed to acquire child stdout".to_string()))?;

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

        // Spawn a background reader that parses each line as a JSON-RPC response
        // and routes it to the waiting oneshot sender.
        spawn_reader_task(BufReader::new(stdout), Arc::clone(&pending));

        let client = Self {
            _child: child,
            stdin: Mutex::new(stdin),
            pending,
            next_id: Mutex::new(1),
            tool_schemas: Vec::new(),
        };

        client.initialize().await?;
        let schemas = client.list_tools().await?;

        Ok(Self {
            tool_schemas: schemas,
            ..client
        })
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    async fn next_id(&self) -> u64 {
        let mut guard = self.next_id.lock().await;
        let id = *guard;
        *guard += 1;
        id
    }

    async fn send_request(&self, method: &'static str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id().await;
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };

        let mut line =
            serde_json::to_string(&request).map_err(|e| McpError::Serialization(e.to_string()))?;
        line.push('\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| McpError::Transport(format!("stdin write: {e}")))?;
        }

        let response = rx.await.map_err(|_| McpError::ServerExited)?;

        if let Some(err) = response.error {
            return Err(McpError::Protocol(err.message));
        }

        response
            .result
            .ok_or_else(|| McpError::Protocol("response missing 'result'".to_string()))
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, method: &'static str, params: Value) -> Result<(), McpError> {
        #[derive(Serialize)]
        struct Notification {
            jsonrpc: &'static str,
            method: &'static str,
            params: Value,
        }

        let notif = Notification {
            jsonrpc: "2.0",
            method,
            params,
        };

        let mut line =
            serde_json::to_string(&notif).map_err(|e| McpError::Serialization(e.to_string()))?;
        line.push('\n');

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpError::Transport(format!("stdin write: {e}")))?;

        Ok(())
    }

    async fn initialize(&self) -> Result<(), McpError> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "sandakan", "version": "0.1.0" }
        });

        self.send_request("initialize", params).await?;
        self.send_notification(
            "notifications/initialized",
            Value::Object(Default::default()),
        )
        .await
    }

    async fn list_tools(&self) -> Result<Vec<crate::application::ports::ToolSchema>, McpError> {
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
                Some(crate::application::ports::ToolSchema {
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
impl McpClientPort for StdioMcpClient {
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

// ─── Background reader task ───────────────────────────────────────────────────

fn spawn_reader_task(reader: BufReader<ChildStdout>, pending: PendingMap) {
    tokio::spawn(async move {
        let mut lines = reader.lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) if line.trim().is_empty() => continue,
                Ok(Some(line)) => {
                    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                        if let Some(id) = resp.id {
                            let mut map = pending.lock().await;
                            if let Some(tx) = map.remove(&id) {
                                // Receiver may have been dropped (caller gave up); ignore.
                                let _ = tx.send(resp);
                            }
                        }
                        // Notifications (no id) are silently discarded.
                    }
                }
                // EOF — child process exited. Pending senders will receive Err on their rx.
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });
}

// ─── Content extraction helper ────────────────────────────────────────────────

/// MCP tools/call returns `{ "content": [{ "type": "text", "text": "..." }] }`.
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
