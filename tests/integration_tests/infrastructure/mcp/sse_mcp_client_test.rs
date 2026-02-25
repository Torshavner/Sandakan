/// Integration tests for `SseMcpClient`.
///
/// We spin up a minimal Axum HTTP server in-process on a dynamically-assigned
/// port so there is no external dependency. The server speaks the MCP HTTP+SSE
/// wire protocol (POST /message → SSE data: line).
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::response::Response;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use tokio::net::TcpListener;

use sandakan::application::ports::{McpClientPort, McpError};
use sandakan::domain::{ToolCall, ToolCallId, ToolName};
use sandakan::infrastructure::mcp::SseMcpClient;

// ─── Minimal in-process MCP SSE server ───────────────────────────────────────

async fn mcp_handler(
    State(_): State<Arc<()>>,
    Json(body): Json<Value>,
) -> Response<axum::body::Body> {
    let id = body["id"].as_u64().unwrap_or(0);
    let method = body["method"].as_str().unwrap_or("");

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "serverInfo": {"name": "test-sse", "version": "0.1.0"}
        }),
        "tools/list" => json!({
            "tools": [{
                "name": "greet",
                "description": "Returns a greeting.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"name": {"type": "string"}},
                    "required": ["name"]
                }
            }]
        }),
        "tools/call" => {
            let name_arg = body["params"]["arguments"]["name"]
                .as_str()
                .unwrap_or("stranger");
            json!({
                "content": [{"type": "text", "text": format!("Hello, {name_arg}!")}]
            })
        }
        _ => json!({"error": {"code": -32601, "message": "Method not found"}}),
    };

    let rpc_resp = json!({"jsonrpc": "2.0", "id": id, "result": result});
    let sse_body = format!("data: {}\n\n", serde_json::to_string(&rpc_resp).unwrap());

    Response::builder()
        .header("Content-Type", "text/event-stream")
        .body(axum::body::Body::from(sse_body))
        .unwrap()
}

async fn start_test_server() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind to random port");
    let addr = listener.local_addr().unwrap();

    let app = Router::new()
        .route("/message", post(mcp_handler))
        .with_state(Arc::new(()));

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn given_sse_server_when_client_initialises_then_tool_schemas_populated() {
    let addr = start_test_server().await;
    let endpoint = format!("http://{addr}");

    let client = SseMcpClient::new(&endpoint)
        .await
        .expect("SseMcpClient::new should succeed");

    assert!(
        !client.tool_schemas.is_empty(),
        "tools/list should return at least one schema"
    );
    assert_eq!(client.tool_schemas[0].name, "greet");
}

#[tokio::test]
async fn given_sse_server_when_call_tool_then_returns_greeting() {
    let addr = start_test_server().await;
    let endpoint = format!("http://{addr}");

    let client = SseMcpClient::new(&endpoint)
        .await
        .expect("SseMcpClient::new should succeed");

    let call = ToolCall {
        id: ToolCallId::new("call-99"),
        name: ToolName::new("greet"),
        arguments: json!({ "name": "Alice" }),
    };

    let result = client
        .call_tool(&call)
        .await
        .expect("call_tool should succeed");

    assert_eq!(result.content, "Hello, Alice!");
}

#[tokio::test]
async fn given_unreachable_endpoint_when_sse_client_created_then_transport_error_returned() {
    // Port 1 is privileged and never listening in test environments.
    let result = SseMcpClient::new("http://127.0.0.1:1").await;

    assert!(
        matches!(result, Err(McpError::Transport(_))),
        "expected Err(Transport(_))"
    );
}
