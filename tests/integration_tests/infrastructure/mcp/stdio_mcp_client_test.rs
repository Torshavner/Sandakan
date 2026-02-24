/// Integration tests for `StdioMcpClient`.
///
/// These tests use a tiny in-process echo server written as a shell one-liner so
/// they remain offline (no network) and require only `sh` + `cat` / `printf`.
/// The MCP wire protocol is exercised end-to-end over real stdin/stdout pipes.
use std::collections::HashMap;

use sandakan::infrastructure::mcp::StdioMcpClient;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Build a minimal MCP stdio server script that:
///  1. Reads one line of JSON-RPC from stdin.
///  2. Responds with a fixed `initialize` result.
///  3. Reads the `notifications/initialized` notification (no response).
///  4. Reads the `tools/list` request, responds with `echo_tool`.
///  5. Reads a `tools/call` request, echoes the `query` argument back.
///
/// Written as a Python one-liner to be portable across macOS / Linux without
/// requiring Node.js or any specific toolchain.
fn echo_server_script() -> String {
    // The script intentionally writes bare newline-delimited JSON (no headers).
    r#"
import sys, json

def respond(id_, result):
    msg = {"jsonrpc": "2.0", "id": id_, "result": result}
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue

    method = req.get("method", "")
    id_ = req.get("id")

    if method == "initialize":
        respond(id_, {"protocolVersion": "2024-11-05", "capabilities": {}, "serverInfo": {"name": "echo", "version": "0.1.0"}})

    elif method == "notifications/initialized":
        # Notifications have no id and expect no response.
        pass

    elif method == "tools/list":
        respond(id_, {
            "tools": [{
                "name": "echo",
                "description": "Echoes its input.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"]
                }
            }]
        })

    elif method == "tools/call":
        args = req.get("params", {}).get("arguments", {})
        text = args.get("query", "(empty)")
        respond(id_, {
            "content": [{"type": "text", "text": f"echo: {text}"}]
        })
"#.to_string()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn given_echo_stdio_server_when_client_initialises_then_tool_schemas_populated() {
    let script = echo_server_script();
    let env = HashMap::new();

    let client = StdioMcpClient::new("python3", &["-c".to_string(), script], &env)
        .await
        .expect("StdioMcpClient::new should succeed");

    assert!(
        !client.tool_schemas.is_empty(),
        "tools/list should return at least one schema"
    );
    assert_eq!(
        client.tool_schemas[0].name, "echo",
        "first tool should be named 'echo'"
    );
}

#[tokio::test]
async fn given_echo_stdio_server_when_call_tool_then_returns_echoed_content() {
    use sandakan::application::ports::McpClientPort;
    use sandakan::domain::{ToolCall, ToolCallId, ToolName};

    let script = echo_server_script();
    let env = HashMap::new();

    let client = StdioMcpClient::new("python3", &["-c".to_string(), script], &env)
        .await
        .expect("StdioMcpClient::new should succeed");

    let call = ToolCall {
        id: ToolCallId::new("call-1"),
        name: ToolName::new("echo"),
        arguments: serde_json::json!({ "query": "hello world" }),
    };

    let result = client
        .call_tool(&call)
        .await
        .expect("call_tool should succeed");

    assert_eq!(result.content, "echo: hello world");
}

#[tokio::test]
async fn given_nonexistent_command_when_stdio_client_created_then_transport_error_returned() {
    let env = HashMap::new();
    let result = StdioMcpClient::new("__no_such_binary__", &[], &env).await;

    assert!(
        matches!(
            result,
            Err(sandakan::application::ports::McpError::Transport(_))
        ),
        "expected Err(Transport(_))"
    );
}
