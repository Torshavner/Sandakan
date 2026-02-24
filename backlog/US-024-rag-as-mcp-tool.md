# Agent: RAG as MCP Tool (`rag_search`)

## Status: BACKLOG

## Requirement Definition

As a **RAG System Developer**, I need **the agent to optionally query the uploaded document corpus as an MCP tool** so that **the LLM can decide when and how many times to search the knowledge base during a ReAct loop, combining domain knowledge with web search in a single agentic turn**.

---

## Context

US-021 shipped the ReAct agentic loop with `POST /api/v1/agent/chat`. The agent can call MCP tools (e.g. `web_search`) but has zero knowledge of the uploaded document corpus. Two endpoints intentionally coexist:

- `POST /api/v1/chat` — RAG-only, always searches the knowledge base
- `POST /api/v1/agent/chat` — agentic, tool-calling, no KB access by default

This story adds an optional `rag_search` `ToolHandler` to the agent endpoint. When enabled, the LLM decides when to call it and can call it multiple times per turn (e.g. query broadly then refine).

**Design constraint — no nested LLM synthesis:** `RetrievalService::query()` runs a full pipeline: embed → search → LLM generation. Exposing that via the port would create a double-LLM chain: a second model synthesises an answer, which the agent LLM then re-synthesises. This doubles token cost and latency while degrading reasoning quality (the agent receives a pre-digested summary instead of raw evidence). The port therefore exposes only the retrieval half of the pipeline.

---

## Architecture

`RetrievalService<L,V>` is a concrete generic struct with no trait abstraction. A new thin `RetrievalServicePort` trait is introduced in L2 ports so the L3 adapter can hold `Arc<dyn RetrievalServicePort>` without coupling to generic parameters or violating hexagonal boundaries.

**`conversation_id` is NOT passed** — the agent manages its own conversation via `AgentService::persist_turn()`. Passing it would cause duplicate message persistence.

**Return type from port is `Vec<SourceChunk>` (raw chunks, no LLM answer)** — formatting belongs in the adapter's `format_rag_response()` function, mirroring `format_brave_results()` in `WebSearchAdapter`. The agent LLM receives raw evidence and writes its own synthesis. The port stays cleanly testable.

```
L2 Ports:   RetrievalServicePort::search_chunks(&str) → Result<Vec<SourceChunk>, RetrievalError>
                    ↑ implemented by
L2 Service: RetrievalService<L,V>::search_chunks(question)  ← embed + search + filter only, NO llm_client call
                    ↑ held as Arc<dyn ...> by
L3 Tools:   RagSearchAdapter implements ToolHandler
                    ↑ registered via
L4 Main:    settings.agent.rag_search_enabled (default false)
```

---

## Layer Responsibilities (Hexagonal, maintained)

| Layer | Change |
|---|---|
| **L1 Domain** | None |
| **L2 Ports** | New `src/application/ports/retrieval_service_port.rs` — `RetrievalServicePort` trait |
| **L2 Services** | `retrieval_service.rs` — new inherent `search_chunks()` method + `impl RetrievalServicePort` |
| **L3 Infrastructure** | New `src/infrastructure/tools/rag_search_adapter.rs` — `RagSearchAdapter` implements `ToolHandler` |
| **L4 Presentation** | `AgentSettings.rag_search_enabled: bool` (default `false`); conditional wiring in `main.rs` |

---

## Implementation Notes

### `RetrievalServicePort` (L2 port)
```rust
#[async_trait]
pub trait RetrievalServicePort: Send + Sync {
    async fn search_chunks(&self, query: &str) -> Result<Vec<SourceChunk>, RetrievalError>;
}
```

### `RetrievalService::search_chunks` (new inherent method, L2 service)

Runs only the retrieval half of the pipeline — **no `llm_client` call**:

1. `embedder.embed(query)`
2. `vector_store.search(&embedding, self.top_k)`
3. Return empty vec if no results or best score < `similarity_threshold`
4. Filter by `similarity_threshold`, apply token-budget trim (same logic as `query()` lines 100–116)
5. Map to `Vec<SourceChunk>` and return

`impl RetrievalServicePort for RetrievalService<L,V>` delegates to this inherent method.

### `RagSearchAdapter` (L3 tool)
- `tool_schema()` — JSON Schema `{ query: string }`, description: *"Search the uploaded knowledge base documents for relevant information. Returns raw source passages. Call multiple times to refine results."*
- `execute()` — extracts `query` arg, calls `port.search_chunks(query)`, formats via `format_rag_response()`
- `format_rag_response(chunks: &[SourceChunk]) -> String`:
  - When chunks is empty: `"No relevant documents found in the knowledge base."`
  - Otherwise:
    ```
    Found {n} relevant sources:
    1. [Page {N}, score: {X.XX}]: {text truncated to 800 chars}
    2. [Page {N}, score: {X.XX}]: {text truncated to 800 chars}
    ```
  - 800-char truncation (not 200) — the agent LLM needs enough substance to reason from, especially for technical PDFs and transcripts.

### `AgentSettings` addition
```rust
#[serde(default)]
pub rag_search_enabled: bool,   // default false — no JSON file changes needed
```

### `main.rs` wiring (inside `if settings.agent.enabled` block)
```rust
if settings.agent.rag_search_enabled {
    let rag = Arc::new(RagSearchAdapter::new(
        Arc::clone(&retrieval_service) as Arc<dyn RetrievalServicePort>,
    ));
    schemas.push(RagSearchAdapter::tool_schema());
    handlers.push(rag as Arc<dyn ToolHandler>);
}
```

---

## File Checklist

| File | Action |
|---|---|
| `src/application/ports/retrieval_service_port.rs` | Create |
| `src/application/ports/mod.rs` | Modify — `mod` + `pub use` + `@AI` map entry |
| `src/application/services/retrieval_service.rs` | Modify — add `search_chunks()` inherent method + `impl RetrievalServicePort` |
| `src/infrastructure/tools/rag_search_adapter.rs` | Create |
| `src/infrastructure/tools/mod.rs` | Modify — `mod` + `pub use` + `@AI` map entry |
| `src/presentation/config/settings.rs` | Modify — add `rag_search_enabled` to `AgentSettings` |
| `src/main.rs` | Modify — import + conditional wiring |
| `tests/unit_tests/infrastructure/tools/mod.rs` | Create |
| `tests/unit_tests/infrastructure/tools/rag_search_adapter_test.rs` | Create |
| `tests/unit_tests/infrastructure/mod.rs` | Modify — add `mod tools` |

---

## Acceptance Criteria

```gherkin
Scenario: Agent queries knowledge base when rag_search_enabled
  Given agent.enabled = true and agent.rag_search_enabled = true
  When a user posts a question about uploaded documents to POST /api/v1/agent/chat
  Then the SSE stream contains a rag_search tool_call progress event
  And the final answer incorporates content from the knowledge base

Scenario: rag_search tool is absent when disabled
  Given agent.enabled = true and agent.rag_search_enabled = false
  When the agent starts
  Then the rag_search tool is not registered in the StaticToolRegistry

Scenario: Agent calls rag_search multiple times in one turn
  Given rag_search is enabled
  When the LLM decides to refine its search with a second query
  Then two rag_search tool_call progress events appear in the SSE stream before the final answer

Scenario: search_chunks does not invoke the LLM client
  Given rag_search is enabled
  When the agent calls rag_search
  Then RetrievalService::search_chunks completes without calling llm_client.complete or llm_client.complete_stream

Scenario: Knowledge base query failure is handled gracefully
  Given rag_search is enabled and the vector store is unavailable
  When the agent calls rag_search
  Then AgentError::Tool is returned with the retrieval error message

Scenario: No relevant documents returns empty-result message
  Given rag_search is enabled and no chunks exceed the similarity threshold
  When the agent calls rag_search
  Then the tool result is "No relevant documents found in the knowledge base."
  And the agent continues its ReAct loop without error

Scenario: Missing query argument is rejected
  Given rag_search is enabled
  When the LLM emits a tool call with no query argument
  Then McpError::Serialization is returned with "missing 'query' argument"
```
