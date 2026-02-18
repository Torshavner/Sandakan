# Contract and Handler Segregation

## Requirement Definition
As a System Developer, I need API contracts separated from handler logic into distinct directories and irrelevant code isolated from the reasoning path so that repository navigability is optimized and LLM context window consumption is minimized.

## Problem Statement
* **Current bottleneck/technical debt:** Mixing HTTP request/response payloads (contracts) with execution logic (handlers) in the Presentation layer creates bloated files that obscure domain intent.
* **Performance/cost implications:** Parsing mixed files forces the ingestion of irrelevant serialization code, increasing LLM API token costs and degrading reasoning performance.
* **Architectural necessity:** Strict separation is required to maintain a Clean Architecture presentation layer, ensuring that handlers only orchestrate logic while contracts define the strictly typed I/O boundaries.

## Acceptance Criteria (Gherkin Enforced)
### Contract Segregation
* **Given** an Axum REST API handler implementation,
* **When** the code is structured,
* **Then** all request and response payload types (contracts) must reside in a separate module directory distinct from the handlers code.

### Navigability Optimization
* **Given** a codebase parsed for domain reasoning,
* **When** evaluating the handler logic,
* **Then** the handler file must only contain routing and dependency orchestration logic.
* **And** the handler file must not contain inline `struct` definitions for data contracts to prevent irrelevant code from polluting the view.

* **Technical Metric:** 100% separation of contract definitions from handler logic; 0 inline structs in handler files.
* **Observability:** `cargo clippy` and `cargo fmt` must pass without warnings, enforcing clean import boundaries.

## Technical Context
* **Architectural patterns:** Clean / Hexagonal Architecture (Layer 4: Presentation).
* **Stack components:** Rust, `axum` (HTTP handlers), `serde` (Serialization).
* **Integration points:** REST API inputs/outputs, Composition Root.
* **Namespace/Config:** Presentation layer grouping (e.g., `presentation/handlers/` and `presentation/contracts/`).

## Cross-Language Mapping
* Rust Axum Handlers & Serde Structs ≈ Spring Boot Controllers & isolated DTO classes.

## Metadata
* **Dependencies:** None
* **Complexity:** Medium
* **Reasoning:** Requires extracting existing API payload structs into new files matching the "One Type, One File" rule, updating visibility via `mod.rs`, and fixing import paths across the Layer 4 Presentation layer.

## Quality Benchmarks
## Test-First Development Plan
- [ ] Parse criteria into Given-When-Then scenarios.
- [ ] Generate failing test suite.
- [ ] Execute test command to confirm failure.
- [ ] Implement minimal logic for pass.
- [ ] Refactor under green state.