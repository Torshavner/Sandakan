# RAG: Precise Media Timestamp Citations

## Requirement Definition
As an end-user, I need the AI's generated answers to include clickable timestamp citations so that I can instantly jump to the exact moment in the original lecture video where the cited concept was discussed.

## Problem Statement
* **Current bottleneck/technical debt:** The current `TranscriptionEngine` returns a single merged `String`, losing all temporal metadata from the Whisper output. This makes deep-linking to source media impossible.
* **Performance/cost implications:** Users spend excessive time manually scrubbing through long-form videos to verify AI claims, leading to lower trust and reduced platform stickiness.
* **Architectural necessity:** The `Domain::Chunk` entity needs to evolve to support temporal metadata to fulfill the "verifiable citations" requirement in the Retrieval Path.

## Acceptance Criteria (Gherkin Enforced)
### Timestamp Extraction & Aggregation
* **Given** the `TranscriptionEngine` returns an array of `TranscriptSegment` objects (text + start/end times),
* **When** the Segment Aggregator groups these segments into a token-budgeted `Chunk`,
* **Then** the `start_time` of the very first segment in that group must be assigned to the `Chunk::start_time` property.

### Vector Database Payload
* **Given** a formatted `Chunk` with a `start_time` and `source_link`,
* **When** the `QdrantAdapter` upserts the chunk into the vector database,
* **Then** both the timestamp and the link must be saved accurately within the Qdrant `PointStruct` JSON payload.

### Frontend Citation Delivery
* **Given** a user submits a RAG query via `POST /api/v1/query`,
* **When** the `RetrievalService` streams the LLM response,
* **Then** the source metadata (link + appended timestamp) must be returned in the response payload for clickable rendering (e.g., `youtube.com/watch?v=XYZ&t=1045s`).

* **Technical Metric:** Timestamps must be accurate to within ±500ms of the original Whisper segment start time.
* **Observability:** Log `citation_metadata_size` in `RetrievalService` to monitor payload overhead.

## Technical Context
* **Architectural patterns:** Clean Architecture (Entity expansion), Adapter Pattern (Qdrant payload mapping).
* **Stack components:** Rust, `serde` for JSON payload serialization, Qdrant gRPC.
* **Integration points:** `domain/entities.rs` (Chunk model), `infrastructure/persistence` (Qdrant), `application/services` (Retrieval).
* **Namespace/Config:** `retrieval.include_metadata: true`.

## Cross-Language Mapping
* `TranscriptSegment` ≈ Subtitle Entry / SRT Block

## Metadata
* **Dependencies:** `20260215_audio_vad_silence_removal.md` (VAD may shift relative timestamps)
* **Complexity:** Medium
* **Reasoning:** Requires a breaking change to the `TranscriptionEngine` trait and a migration of the Qdrant collection schema to support the new metadata fields.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Refactor `TranscriptionEngine` trait; update `infrastructure/llm` to return `Vec<TranscriptSegment>`.
- [ ] Update `domain/chunk.rs` to include `pub start_time: Option<f32>`.
- [ ] Generate failing test in `tests/application/services/retrieval_service_tests.rs` for metadata inclusion.
- [ ] Implement mapping in `QdrantAdapter` and verify via `cargo test`.
- [ ] Refactor `RetrievalService` to ensure the `source_link` is correctly formatted with the `&t=X` suffix.