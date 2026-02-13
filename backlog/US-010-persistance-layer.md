# PostgreSQL Persistence Layer

## Requirement Definition
As a System Architect, I need a persistent storage layer using PostgreSQL and sqlx so that agent Job IDs and Conversation History are preserved across service restarts and multiple user sessions.

## Problem Statement
* **Current bottleneck/technical debt:** All agent interactions are currently ephemeral (in-memory); restarting the server wipes all context and job tracking.
* **Performance/cost implications:** Loss of Job IDs leads to "zombie" background tasks that cannot be queried, resulting in wasted LLM tokens and compute resources.
* **Architectural necessity:** To support long-running RAG tasks and multi-turn dialogues, the system requires a durable "Source of Truth" that adheres to the established Hexagonal Architecture.

## Acceptance Criteria (Gherkin Enforced)
### Database Connectivity & Health
* **Given** a PostgreSQL instance is configured in the environment,
* **When** the application initializes the `tokio` runtime,
* **Then** the `sqlx` connection pool must successfully connect or the application must gracefully exit with a configuration error.

### Job Lifecycle Management
* **Given** a new asynchronous task (e.g., PDF summarization) is triggered,
* **When** the `IngestionService` starts the process,
* **Then** a record is created in the `jobs` table with status `QUEUED` and a unique UUID.

### Conversation Continuity
* **Given** an existing `conversation_id` with 5 previous messages,
* **When** a user submits a new natural language query,
* **Then** the `RetrievalService` must load all 5 messages to augment the LLM context and subsequently persist both the new query and the response.

* **Technical Metric:** Database queries in the read path (History Load) must complete in < 50ms for the 95th percentile.
* **Observability:** Every SQL execution must be wrapped in a `tracing::instrument` span to capture query latency and potential errors.

## Technical Context
* **Architectural patterns:** Repository Pattern (Infrastructure Layer), Layered Dependency (L3 -> L2).
* **Stack components:** Rust, `sqlx` (Postgres driver), `tokio`, PostgreSQL (JSONB for tool calls).
* **Integration points:** `infrastructure/persistence/postgres`, `application/ports/repository_trait.rs`.
* **Namespace/Config:** `DATABASE_URL`, `MAX_CONNECTIONS`.

## Cross-Language Mapping
* `sqlx::Pool<Postgres>` ≈ Connection Manager
* `JSONB` ≈ Serde-serialized Map/Value

## Metadata
* **Dependencies:** None (Foundation layer)
* **Complexity:** Medium
* **Reasoning:** Requires coordination between async traits and concrete database migrations while ensuring `Send + Sync` compatibility for the `sqlx` pool across threads.

## Quality Benchmarks
* **SQL Safety:** All queries must use `sqlx::query!` macros for compile-time type checking.
* **Connection Resilience:** Implement exponential backoff for the initial connection pool logic.

## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Generate failing test suite in `tests/persistence_tests.rs` using `testcontainers` or a dedicated test DB.
- [ ] Execute `cargo test` to confirm failure (connection refused/missing tables).
- [ ] Implement migrations and `PostgresRepository` adapter.
- [ ] Refactor under green state to ensure `sqlx` types do not leak into the Domain layer.