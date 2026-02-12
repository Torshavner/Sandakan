# Token Budget Validation for Retrieval

## Requirement Definition
As a **RAG System Operator**, I need **dynamic token budgeting before LLM context construction** so that **the system prevents "Context Window Exceeded" errors and ensures reliable responses even with heavy retrieval payloads.**

## Problem Statement
* **Current bottleneck/technical debt:** The current `RetrievalService` blindly fetches a fixed number of chunks (Top-K) without calculating the actual token density. Long user queries or dense chunks cause API failures (400 Bad Request).
* **Performance/cost implications:** Wasted GPU/API cycles on requests that are doomed to fail. Potential for partial/malformed responses if the LLM arbitrarily truncates input.
* **Architectural necessity:** To support hot-swapping LLMs (e.g., moving from a 4k context model to a 128k model), the retrieval logic must dynamically adapt to the active model's constraints rather than using hardcoded limits.

## Acceptance Criteria (Gherkin Enforced)

### 1. Dynamic Budget Calculation
* **Given** the active LLM has a total context window of 4096 tokens,
* **And** the `system_prompt` consumes 200 tokens,
* **And** we reserve a safety buffer of 500 tokens for the `output_generation`,
* **When** a user submits a query containing 50 tokens,
* **Then** the `available_retrieval_budget` must be calculated as 3346 tokens (4096 - 200 - 500 - 50).

### 2. Context Truncation Strategy
* **Given** the `available_retrieval_budget` is 1000 tokens,
* **And** the `VectorStore` returns 5 chunks totaling 1500 tokens (sorted by relevance),
* **When** the `RetrievalService` constructs the final context payload,
* **Then** it must include only the top N chunks that fit within the 1000 token limit,
* **And** it must log a warning: "Token budget exceeded: dropped 2 lowest-relevance chunks".

### 3. Edge Case: Zero Budget
* **Given** a user submits a massive query that exceeds the (Context Window - System Prompt),
* **When** the budget is calculated,
* **Then** the system must return a specific `InputValidationError` immediately,
* **And** strictly prevent the expensive Vector DB lookup or LLM call.

* **Technical Metric:** Token counting must be performed using `tiktoken-rs` (or equivalent BPE tokenizer) to ensure >99% alignment with the target LLM's actual counting.
* **Observability:** `tracing` spans must record `tokens.system`, `tokens.query`, `tokens.retrieved`, and `tokens.budget_remaining`.

## Technical Context
* **Architectural patterns:** Pipeline / Filter pattern within `RetrievalService`.
* **Stack components:**
    * **Crate:** `tiktoken-rs` (for BPE tokenization).
    * **Service:** `application/services/retrieval_service.rs`.
    * **Config:** `infrastructure/llm/config.rs` (adds `max_context_window` and `reserved_output_tokens`).
* **Integration points:** Adapters for OpenAI (for model specific limits) and Qdrant (retrieval source).
* **Namespace/Config:** `AppConfig::LlmSettings::context_window_size`.

## Cross-Language Mapping
* `tiktoken-rs::CoreBPE` (Rust) ≈ `tiktoken.Encoding` (Python)
* `Vec<Chunk>` (Rust) ≈ `List[Document]` (LangChain/Python)

## Metadata
* **Dependencies:** `20260210_retrieval_service_basic` (assumed predecessor)
* **Complexity:** Medium
* **Reasoning:** Requires integrating a tokenizer that matches the LLM's specific vocabulary. Logic is CPU-bound and must be efficient to avoid adding latency to the read path.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Add `tiktoken-rs` dependency and create a `TokenCounter` domain service.
- [ ] Generate failing test: `given_huge_query_when_retrieving_then_returns_error`.
- [ ] Generate failing test: `given_small_budget_when_retrieving_then_truncates_chunks`.
- [ ] Implement `calculate_budget` logic in `RetrievalService`.
- [ ] Implement iterative chunk selection loop.
- [ ] Refactor observability to include token breakdown fields.