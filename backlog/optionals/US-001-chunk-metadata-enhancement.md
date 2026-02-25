# Metadata-Enriched Indexing Pipeline

## Requirement Definition
As a System Architect, I need to implement a metadata-injection layer during the RAG ingestion process so that retrieved chunks contain global context, leading to higher retrieval precision and verifiable citations for the end user.

## Problem Statement
* **Current bottleneck/technical debt:** Current chunks are "orphaned" text fragments; vector search retrieves semantically similar but contextually incorrect data (e.g., retrieving 2021 financial data for a 2023 query).
* **Performance/cost implications:** Without metadata filtering, the LLM processes irrelevant context, increasing token costs and increasing the risk of hallucinations by ~30% in multi-document environments.
* **Architectural necessity:** To support "Hybrid Search" and enterprise-grade traceability, the system must transition from raw text chunks to structured, context-aware entities.

## Acceptance Criteria (Gherkin Enforced)
### Contextual Embedding Generation
* **Given** a raw text chunk and its associated `DocumentMetadata`,
* **When** the `as_contextual_string()` method is invoked,
* **Then** the system must produce a formatted string (Title + Page + Content) to "color" the vector embedding without duplicating storage in the database.

### Memory-Efficient Metadata Storage
* **Given** multiple chunks belonging to the same parent document,
* **When** stored in the application heap,
* **Then** the system must use `Arc<DocumentMetadata>` to ensure document-level strings (like Title) exist only once in memory.

### Traceable Retrieval
* **Given** a successful vector search result from Qdrant,
* **When** the context is injected into the LLM prompt,
* **Then** it must include the `source_url` and `page_number` from the metadata to enable UI citations.

* **Technical Metric:** Memory overhead for document-level metadata must be reduced by $N-1$ (where $N$ is chunk count) using pointer-shared references.
* **Observability:** Each `tracing` span for ingestion must log the `document_id` and `chunk_count` for auditability.

## Technical Context
* **Architectural patterns:** Hexagonal Architecture (Infrastructure Adapters for Qdrant), Flyweight Pattern (for shared metadata).
* **Stack components:** Rust, `tokio` (Async), `Arc<T>` (Atomic Reference Counting), `candle` (Inference).
* **Integration points:** Qdrant (Vector Store), OpenAI/Local LLM (Embeddings).
* **Namespace/Config:** `metadata.injection_template = "Title: {t}\nPage: {p}\nContent: {c}"`

## Cross-Language Mapping
* `Arc<T>` (Rust) ≈ `Shared Memory / Singleton Reference` (General)
* `as_contextual_string` ≈ `Compute-on-demand Property`

## Metadata
* **Dependencies:** 20260212_initial_rag_setup | None
* **Complexity:** Medium
* **Reasoning:** Requires refactoring the `Chunk` struct to handle shared ownership (`Arc`) and updating the ingestion logic to separate embedding-input from storage-payload.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Generate failing test suite: `given_identical_text_in_different_docs_when_embedded_then_vectors_must_differ`.
- [ ] Execute `cargo test` to confirm failure.
- [ ] Implement `as_contextual_string` and `Arc<DocumentMetadata>` logic.
- [ ] Refactor under green state to ensure no `clone()` calls on metadata.