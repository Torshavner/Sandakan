# Test Guidelines

To maintain high velocity, prevent context collapse, and ensure deterministic environments, these rules are **non-negotiable**.

## 1. Test Discovery & Refactoring

**Context is currency.** To ensure the AI never loses the thread when dealing with large test suites:

* **Refactoring Trigger:** When a test file becomes too large (refer to the global file length limits in `code.guidelines.md`), you **must** split it by sub-feature or move shared setup and fixtures to `tests/helpers/`.
* **Discovery Rule:** Before reading a complex test file, map the existing tests using:
`grep -E '^\s*#\[(tokio::)?test\]|^\s*fn ' <filepath>`

## 2. Execution Strategy

Stop the noise. Target strictly.

* **Targeted Testing:** NEVER run `cargo test` globally unless explicitly requested. Always target:
`cargo test --test mod -- <test_file>  -- <test_name>` 2>&1
* **Isolate E2E Execution:** E2E tests are slow. They should be run selectively or via CI, not during rapid local iteration unless verifying a specific integration point.

## 3. Architecture, Location, & Intent Distinction

Tests are **strictly decoupled** from implementation files. Do not create `mod tests` blocks inside `src/`. You must distinguish the *intent* of a test strictly by its location and execution boundary following this generic structure:
`tests/<test_type>/<crate_name>/<module_name>/<file_name>_test.rs`

| Test Intent | Location Example | I/O Boundary Constraint |
| --- | --- | --- |
| **Unit** | `tests/unit_tests/domain/embeddings_test.rs` | **Strictly Offline.** No I/O. Tests isolated module logic via its public interface. |
| **Integration** | `tests/integration_tests/presentation/api_test.rs` | **Strictly Offline.** Black-box API orchestration. Uses injected mocks. |
| **E2E** | `tests/e2e_tests/ingestion/pipeline_test.rs` | **Live I/O Allowed.** Must use `testcontainers` for ephemeral DBs/services. |

### The Shared Helper Rule

* **Shared Test Helpers:** NEVER create a `tests/lib.rs`. Cargo will treat it as a standalone executable and fail. To share code between these test files, put it in `tests/helpers/mod.rs` (and submodules like `tests/helpers/database.rs`), then declare `mod helpers;` at the top of the specific test files that need it.

> **Agentic Rule:** When modifying logic in `src/<module>/<file>.rs`, you MUST check the corresponding `tests/unit_tests/<crate>/<module>/<file>_test.rs` file to update breaking assertions first.

## 4. Strict BDD Naming Convention

Test function names act as documentation and MUST strictly adhere to the Given-When-Then structure using **`snake_case`**.

* **Format:** `given_<state_or_context>_when_<action_taken>_then_<expected_outcome>`
* **The 5-Word Rule:** If a test name is shorter than 5 words, it is too vague and must be expanded.
* **Example:** `#[tokio::test] async fn given_running_postgres_container_when_inserting_user_then_persists_to_db() { ... }`

## 5. Mocking vs. Testcontainers

* **Unit & Integration (Hand-Written Stubs):** Prefer lightweight, hand-written, in-memory stubs (e.g., `struct MockRepo`) over heavy macro-based frameworks like `mockall`. They are easier for an LLM to read and modify.
* **E2E (The `testcontainers` Rule):** E2E tests MUST NOT hit shared, persistent, or live remote databases. You **must** use the `testcontainers` crate to spin up isolated, ephemeral Docker containers for the duration of the test. To prevent port collisions and ensure parallel test execution, always use dynamically assigned host ports (e.g., `.get_host_port_ipv4()`) rather than hardcoding static ports like `5432`.

## 6. Assertions & Safety Exceptions

* **Explicit/Structural Assertions:** Verify intent using structural matching over generic string matching.
* *Use:* `assert!(matches!(result, Err(DomainError::InvalidState)))`
* **No Silent Failures:** Negative tests must actually verify the specific type of error returned, rather than just checking `.is_err()`.
* **The `unwrap()` Exception:** Unlike production code, `.unwrap()` and `.expect()` are ENCOURAGED in test setup to fail fast. Do not waste tokens on complex `Result` boilerplate in test files.
