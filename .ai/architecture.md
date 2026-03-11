# Architecture

## 1. High-Level Design Philosophy

This architecture follows **Hexagonal (Ports & Adapters)** principles to ensure the AI agent can navigate, mock, and test components in isolation. It prioritizes **context window efficiency** by enforcing small, specialized files and a searchable domain graph.

* **Core Principle:** Dependencies point inward (**L4 → L3 → L2 → L1**).
* **Concurrency:** Non-blocking async I/O via `tokio` with dedicated background workers for heavy lifting (Ingestion).

---

## 2. Layered Structure & Context Boundaries

To prevent "Context Collapse," the system is partitioned into four distinct layers. Layer logic and testing boundaries are strictly defined.

### L1: Domain (The Core)

* **Role:** Pure business logic and Value Objects. **Strictly No I/O.**
* **Key Entities:** `Chunk`, `Document`, `Job`, `Conversation`, `Message`, `EvalEvent`, `EvalOutboxEntry`.
* **Constraint:** Uses the **Newtype Pattern** (e.g., `struct ChunkId(Uuid)`) to prevent primitive obsession and ensure type-safe ID handling across the pipeline.
* **Testing Bound:** Pure offline Unit tests inside `mod tests`.

### L2: Application (The Orchestrator)

* **Role:** Defines **Ports (Traits)** and Services.
* **Ports:** `VectorStore`, `Embedder`, `LlmClient`, `TextSplitter`, `TranscriptionEngine`, `EvalEventRepository`, `EvalOutboxRepository`.
* **Services:** `IngestionService` (Sync flow), `RetrievalService` (RAG logic), `AgentService` (ReAct loop), `EvalWorker` (Background scoring), `EvalRunner` (Offline CLI), `TokenCounter`.
* **Testing Bound:** Offline Unit/Integration tests using hand-written, in-memory mocks/stubs. No heavy macro frameworks.

### L3: Infrastructure (The Adapters)

* **Role:** Concrete implementations of L2 Ports.
* **Adapters:** `QdrantAdapter`, `PgRepository`, `OpenAiClient`, `CandleLocalInference`.
* **Testing Bound:** E2E testing using `testcontainers` with dynamic port assignment. No mocked integration tests belong here.

### L4: Presentation (The Composition Root)

* **Role:** Axum handlers, CLI entry points, and global `AppState`.
* **Constraint:** Handlers are **Zero-DTO**. They only extract data, call an L2 Service, and map the response. DTOs must live in a separate `schema` or `contract` module.

---

## 3. Data Workflows & Execution Boundaries

The pipeline distinguishes between **Immediate API response** and **Deferred background processing** to maintain system responsiveness.

### The Ingestion Pipeline (Write Path)

1. **Entry:** `POST /api/v1/ingest` validates the multipart upload.
2. **Handoff:** A `Job` is created in Postgres (Status: `Queued`).
3. **Worker:** An `IngestionWorker` (Background Actor) consumes the task via `mpsc` channel.
4. **Process:** Routing via `CompositeFileLoader` → `TextSplitter` → `Embedder` → `VectorStore`.

### The Agentic Pipeline (ReAct Loop)

1. **Entry:** `POST /api/v1/agent/chat` initiates `AgentService::run()`.
2. **Reason:** LLM called with `tool_choice: auto`; if a tool call is returned, dispatch it.
3. **Act:** `ToolRegistry` resolves the tool (RAG search, FS, web, MCP) and executes with a configurable timeout.
4. **Observe:** Tool result appended to message history; loop repeats.
5. **Terminate:** `finish_reason = stop`, no tool calls returned, or `max_iterations` reached.
6. **Reflect:** `CriticEvaluator` scores the final answer (0.0–1.0) on completeness, grounding, and clarity.
7. **Output:** Progress events streamed via SSE throughout.

**Tool Registry modes:** `StaticToolRegistry` (fixed list) or `SemanticToolRegistry` (dynamic selection via embedding similarity, controlled by `agent.semantic_tools`). MCP tools are merged transparently via `CompositeMcpClient`.

### The Retrieval Pipeline (Read Path)

1. **Search:** User query is embedded (dense) and optionally sparse-embedded (BM25).
2. **Vector Search:** `VectorStore.search()` (dense) or `search_hybrid()` (dense + sparse → Qdrant RRF fusion) when `qdrant.hybrid_search = true`.
3. **Augmentation:** Context chunks ranked by token budget; metadata-enriched via `chunk.as_contextual_string()`.
4. **Generation:** Streamed tokens returned via SSE using `tokio::select!` for keep-alive management.
5. **Eval Capture (optional):** When `eval.enabled = true`, `RetrievalService` fire-and-forgets an `EvalEvent` + `eval_outbox` row for background scoring.

### Eval Background Worker

Gated behind `eval.enabled` feature flag (default `false`). When enabled:

1. **Outbox Pattern:** Each RAG query inserts an `eval_outbox` row (status: `pending`) alongside the `eval_events` row. Durable — survives restarts.
2. **EvalWorker:** Background actor spawned at startup. Polls `eval_outbox` via `FOR UPDATE SKIP LOCKED` at configurable intervals (`eval.worker_poll_interval_secs`, default 30s).
3. **Scoring:** For each claimed entry, loads the `EvalEvent`, runs LLM-as-judge faithfulness scoring via `eval_metrics::compute_faithfulness()`, emits structured `tracing::info!` with metric fields (`eval.result`), marks outbox row `done` or `failed`.
4. **US-017 Ready:** `EvalWorker` separates `receive_batch()` (transport concern — outbox polling) from `process_entry()` (stable business logic). When the broker abstraction lands, `receive_batch()` extracts into `OutboxSubscriber<EvalOutboxEntry>::receive()` — mechanical refactor.
5. **Observability:** Eval results emitted as structured tracing events. A future story adds the Loki/Vector/Grafana stack to consume them.

---

## 4. Agentic Interaction Rules

When modifying this architecture, the AI Agent must follow these state-management rules, cross-referencing Code and Test guidelines:

| Action | Required Architectural Step |
| --- | --- |
| **New Domain Added** | Create folder, add `mod.rs`|
| **New Provider Added** | Implement the L2 Trait in L3 and update the corresponding `Factory`. |
| **Navigating Large Files** | Respect the `// @AI-BYPASS-LENGTH` header for configurations; otherwise, adhere to the file size limits and refactor triggers defined in Code Guidelines. |
| **Testing Intent** | Route test execution intents strictly by directory: `mod tests` (Unit), `tests/integration/` (Offline/Mocked API), `tests/e2e/` (Containers). |
| **Deferred Work** | Ignore any roadmap items or tasks tagged with `[DEFERRED]` or `[IGNORE]`. |

---

## 5. Technical Stack Summary

| Component | Technology | Intent |
| --- | --- | --- |
| **Runtime** | `tokio` | Async execution & task spawning. |
| **API** | `axum` | Type-safe routing & middleware. |
| **Database** | `PostgreSQL` + `sqlx` | Persistent state & job tracking. |
| **Vector** | `Qdrant` | Semantic search & high-dimensional indexing. |
| **Inference** | `candle` | Local, private embeddings & transcription. |
| **Sparse Search** | `Bm25SparseEmbedder` | TF tokenizer + FNV-1a hash → Qdrant sparse vectors. |
| **Hybrid Fusion** | Qdrant RRF | `PrefetchQueryBuilder` + `Fusion::Rrf` merges dense & sparse. |
| **Agent Loop** | ReAct + Critic | `AgentService`: Reason → Act → Observe; `CriticEvaluator` scores quality. |
| **Tool Protocol** | MCP | `CompositeMcpClient` fans out to stdio/SSE MCP servers. |
| **Observability** | `tracing` + OTLP | Structured logging + OpenTelemetry export to Tempo. |

