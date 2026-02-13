# Streaming Chat Completions (Rig & Persistence)

## Requirement Definition
As a Frontend Consumer (e.g., Open WebUI), I need the Chat API to support Server-Sent Events (SSE) streaming via the Rig Agent framework, while ensuring the final complete message is persisted to PostgreSQL, so that users perceive immediate responsiveness (low Time-to-First-Token) without losing conversation history.

## Problem Statement
* **Current bottleneck:** The system currently waits for the full LLM generation to complete before sending any data, causing "hanging" UI states on long responses.
* **Performance/cost implications:** High latency perception degrades user trust; failed streams currently result in lost context if not handled correctly.
* **Architectural necessity:** Aligning `rig-core` streaming capabilities with the `axum` HTTP layer and the `sqlx` persistence layer is critical for a production-ready agent.

## Acceptance Criteria (Gherkin Enforced)
### Streaming Transport
* **Given** a valid HTTP POST request to `/v1/chat/completions` with `"stream": true`,
* **When** the Rig Agent begins generating tokens,
* **Then** the server must immediately yield `text/event-stream` events compliant with the OpenAI chunk format.

### Persistence Strategy (Stream Aggregation)
* **Given** a streaming response is in progress,
* **When** the stream completes successfully (stops generating),
* **Then** the system must aggregate all streamed chunks into a single string and atomically commit it to the `messages` table in PostgreSQL.

### Error Handling & Partial State
* **Given** a stream that fails mid-generation (e.g., LLM timeout),
* **When** the connection drops,
* **Then** the system must log the error and attempt to save the *partial* response to the DB to preserve the record of the attempt.

* **Technical Metric:** Time to First Token (TTFT) < 800ms.
* **Observability:** Trace ID must persist across the stream lifecycle; log final token count upon completion.

## Technical Context
* **Architectural patterns:** Pass-Through Aggregator (Stream to client + Buffer for DB).
* **Stack components:** `rig-core` (Agent Streaming), `axum` (SSE), `sqlx` (PostgreSQL), `tokio::sync::mpsc`.
* **Integration points:**
    * `Application::ChatService`: Orchestrates Rig Agent.
    * `Infrastructure::PostgresRepository`: Saves final state.
* **Namespace/Config:** `SSE_KEEP_ALIVE_INTERVAL`.

## Cross-Language Mapping
* `rig::agent::Agent::stream(...)` ≈ LangChain `.stream()`
* `axum::response::Sse` ≈ Flask/FastAPI `StreamingResponse`

## Metadata
* **Dependencies:** US-009 (PostgreSQL Persistence)
* **Complexity:** High
* **Reasoning:** Requires managing dual-state: pushing async bytes to the HTTP socket while simultaneously buffering them for a blocking (async) DB write at the end of the lifecycle.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Create a mock Rig agent that yields a fixed stream of tokens.
- [ ] Implement an `axum` handler that consumes the mock stream and validates SSE headers.
- [ ] Add the "Sidecar" logic: Accumulate tokens in a `String` buffer during iteration.
- [ ] Implement the `Drop` or `Complete` handler to trigger the `repository.save_message` call.
- [ ] Verify DB contents match the streamed output.