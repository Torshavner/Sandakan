# Agent: Real MCP Wire Protocol (stdio / SSE Transport)

## Status: BACKLOG

## Requirement Definition

As a **RAG System Developer**, I need **the agent to communicate with external MCP servers over the real MCP wire protocol (stdio or HTTP+SSE transport)** so that **any compliant MCP server (browser automation, database tools, custom services) can be plugged in without writing a Rust adapter**.

---

## Context

US-021 shipped `StandardMcpAdapter` as a local handler dispatch — tool calls are routed to `Arc<dyn ToolHandler>` implementations compiled into the binary (e.g. `WebSearchAdapter`). This is marked `// TODO: US-022/US-023` in source.

The [Model Context Protocol](https://modelcontextprotocol.io) defines two transports:
- **stdio** — parent process spawns child MCP server, communicates over stdin/stdout with JSON-RPC 2.0
- **HTTP+SSE** — client connects to MCP server's HTTP endpoint; requests via POST, responses via SSE stream

A real MCP client replaces `StandardMcpAdapter` while keeping `McpClientPort` unchanged — no changes to `AgentService` or any layer above L3.

---

## Architecture

### New: `src/infrastructure/mcp/stdio_mcp_client.rs`

Spawns a child process via `tokio::process::Command`, writes JSON-RPC requests to stdin, reads responses from stdout.

```rust
pub struct StdioMcpClient {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout_reader: tokio::io::BufReader<tokio::process::ChildStdout>,
    pending: HashMap<u64, oneshot::Sender<JsonRpcResponse>>,
}
```

Lifecycle: `initialize` → `tools/list` → per-call `tools/call` → `shutdown` on drop.

### New: `src/infrastructure/mcp/sse_mcp_client.rs`

HTTP POST to `{endpoint}/message`, reads SSE stream for the response. Stateless per call — no persistent connection required.

### Config: `appsettings.*.json`

```json
"agent": {
  "mcp_servers": [
    { "name": "brave-search", "transport": "stdio", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-brave-search"], "env": { "BRAVE_API_KEY": "${BRAVE_API_KEY}" } },
    { "name": "filesystem",   "transport": "stdio", "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"] }
  ]
}
```

### Tool registry

When MCP servers are configured, `tools/list` is called at startup (once per server) to populate `StaticToolRegistry`. No code changes to `AgentService` or `ToolRegistry` trait.

---

## Layer Responsibilities (Hexagonal, maintained)

| Layer | Change |
|---|---|
| **L1 Domain** | None |
| **L2 Ports** | `McpClientPort` unchanged |
| **L2 Services** | `AgentService` unchanged |
| **L3 Infrastructure** | Add `stdio_mcp_client.rs`, `sse_mcp_client.rs`; update `mcp/mod.rs` |
| **L4 Presentation** | `AgentSettings.mcp_servers: Vec<McpServerConfig>`; factory logic in `main.rs` |

---

## Deferred

- MCP authentication (OAuth, API key headers) — follow-up story.
- Dynamic tool registration (servers that change their tool list at runtime) — out of scope.

---

## Acceptance Criteria

```gherkin
Scenario: stdio MCP server is called via wire protocol
  Given an MCP server is configured with transport "stdio" in appsettings
  When a user posts a message that triggers a tool call
  Then the agent spawns the configured process and calls tools/call over stdin/stdout

Scenario: SSE MCP server is called via wire protocol
  Given an MCP server is configured with transport "sse" and an endpoint URL
  When a user posts a message that triggers a tool call
  Then the agent POSTs to the MCP server endpoint and reads the SSE response

Scenario: Tool list is populated from MCP server at startup
  Given one or more MCP servers are configured
  When the application starts
  Then tools/list is called on each server and the results populate the StaticToolRegistry

Scenario: MCP server process crash is handled gracefully
  Given an stdio MCP server process exits unexpectedly
  When the agent attempts a tool call
  Then AgentError::Tool is returned and the server process is not re-spawned silently
```
