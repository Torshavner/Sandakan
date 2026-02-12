# Configurable Embedding Service

## Requirement Definition
As a **System Architect/Developer**, I need **a configurable abstraction for vector embedding generation** so that **I can seamlessly switch between cost-effective local inference (Candle) during development and high-performance remote APIs (OpenAI) in production without refactoring the codebase.**

## Problem Statement
* **Current bottleneck/technical debt:** Hardcoded dependencies on specific embedding providers lock the system into a single vendor and make local development dependent on internet connectivity and API keys.
* **Performance/cost implications:** Running full test suites or local dev environments against paid APIs (OpenAI) incurs unnecessary costs and latency.
* **Architectural necessity:** To strictly adhere to Hexagonal Architecture, the Domain layer must rely on an abstract `Embedder` port, not concrete implementation details of specific models.

## Acceptance Criteria (Gherkin Enforced)
### Strategy Configuration
* **Given** a `config.json` file specifying `strategy: "local"` and `model: "all-MiniLM-L6-v2"`,
* **When** the application initializes the `IngestionService`,
* **Then** the system loads the `LocalCandleEmbedder` adapter and downloads weights from HuggingFace if missing.

### Remote API Integration
* **Given** a `config.json` file specifying `strategy: "openai"` and `model: "text-embedding-3-small"`,
* **When** the application initializes,
* **Then** the system loads the `OpenAIEmbedder` adapter and validates the presence of the `OPENAI_API_KEY` environment variable.

### Interface Consistency
* **Given** an instantiated Embedder (either Local or Remote),
* **When** the `embed(text)` method is called with a string,
* **Then** it returns a `Result<Vec<f32>>` matching the configured `dimension` (e.g., 384 for MiniLM, 1536 for OpenAI).

* **Technical Metric:** Local inference latency < 200ms for short chunks on standard CPU.
* **Observability:** Logs must indicate which Strategy was loaded at startup (e.g., "🕯️ Loading Local Candle Model").

## Technical Context
* **Architectural patterns:** Strategy Pattern, Hexagonal Architecture (Port/Adapter), Dependency Injection.
* **Stack components:** Rust, `candle-core` / `candle-transformers` (Local), `reqwest` (Remote), `async_trait`.
* **Integration points:** Hugging Face Hub (for model weights), OpenAI API endpoint.
* **Namespace/Config:** `application.embedding.strategy` (Enum: `local`, `openai`).

## Cross-Language Mapping
* `Trait` (Rust) ≈ `Interface` (Java/C#)
* `Enum Dispatch` (Rust) ≈ `Polymorphism` / `Factory Pattern` (OOP)

## Metadata
* **Dependencies:** None (Core Infrastructure)
* **Complexity:** Medium
* **Reasoning:** Involves integrating ML inference libraries (Candle) which requires managing large binary assets (weights) and potential hardware acceleration (Metal/CUDA).

## Quality Benchmarks
## Test-First Development Plan
- [ ] Define `Embedder` trait in `application/ports`.
- [ ] Create `tests/embedder_factory_test.rs` validating config parsing.
- [ ] Implement `OpenAIEmbedder` struct with mocked HTTP client.
- [ ] Implement `LocalCandleEmbedder` struct with `candle-transformers`.
- [ ] Verify `Vec<f32>` dimensions match config in integration tests.