# Agent: Real Token Streaming + Parallel Tool Execution

## Status: BACKLOG

## Requirement Definition

As a **RAG System Developer**, I need **the agent endpoint to stream the final answer token-by-token and execute independent tool calls in parallel** so that **users see a responsive streaming UX and round-trip latency is minimised when multiple tools are invoked**.

---

## Context

US-021 shipped the ReAct agentic loop with two known limitations (marked `// TODO: US-022` in source):

1. **Single-chunk final answer** — `AgentService::run_react_loop()` wraps the final `Content` string in `futures::stream::once(...)`. The full response is buffered inside `complete_with_tools()` before any token reaches the client. A second `complete_stream()` call after the loop would yield real token-by-token SSE output.

2. **Sequential tool execution** — when the LLM returns multiple `ToolCalls` in one response, they are executed one at a time (`for call in calls`). Independent calls (e.g. web_search + calendar_lookup) have no data dependency and can be parallelised with `futures::future::join_all`.

---

## Architecture

### Change 1 — Real streaming final turn

After the ReAct loop exits with `LlmToolResponse::Content`, instead of wrapping the string:

```
current:  stream::once(async { Ok(text) })
target:   llm_client.complete_stream(&messages, system_prompt).await?
```

`complete_stream()` already exists on `LlmClient` and is implemented by `AzureStreamingClient`. The agent needs to reconstruct the final user + assistant history in `messages` and call `complete_stream()` to yield the token stream.

**Impact on `AgentChatResponse`**: `token_stream: LlmTokenStream` is already a `Pin<Box<dyn Stream<...>>>` — no API surface change needed.

### Change 2 — Parallel tool execution

```rust
// current (sequential)
for call in calls { ... mcp_client.call_tool(&call).await? }

// target (parallel)
let results = futures::future::join_all(
    calls.iter().map(|call| mcp_client.call_tool(call))
).await;
// collect results, propagate first error
```

Progress events (`ToolCall` / `ToolResult`) must still be emitted per-tool; use `try_send` per result after `join_all` completes.

---

## Layer Responsibilities (Hexagonal, maintained)

| Layer | Change |
|---|---|
| **L2 Services** | `AgentService::run_react_loop()` — replace `stream::once` with `complete_stream()` call; replace sequential loop with `join_all` |
| **L3 Infrastructure** | No change — `complete_stream()` already implemented |
| **L4 Presentation** | No change — `token_stream` type unchanged |

---

## Deferred

- MCP wire protocol (stdio / SSE transport) — addressed in US-023.
- Streaming progress events mid-tool-execution — out of scope; progress channel already works.

---

## Acceptance Criteria

```gherkin
Scenario: Final answer streams token by token
  Given the agent service is enabled and the LLM supports streaming
  When a user posts a message to POST /api/v1/agent/chat
  Then the SSE response emits multiple token events before the done event

Scenario: Independent tool calls execute in parallel
  Given the LLM returns two tool calls in a single response
  When the agent executes those tool calls
  Then both tool calls are dispatched concurrently and total latency ≈ max(t1, t2) not t1 + t2

Scenario: Parallel tool failure is propagated
  Given the LLM returns two tool calls and one fails
  When the agent executes those tool calls in parallel
  Then AgentError::Tool is returned and no partial result is emitted
```
