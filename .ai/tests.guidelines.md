# Test Guidelines

To maintain high velocity, prevent context collapse, and ensure deterministic environments, these rules are **non-negotiable**.

## 1. Test Discovery & Refactoring

**Context is currency.** To ensure the AI never loses the thread when dealing with large test suites:

* **Refactoring Trigger:** When a test file becomes too large (refer to the global file length limits in `code.guidelines.md`), you **must** split it by sub-feature or move shared setup and fixtures to `tests/common/`.
* **Discovery Rule:** Before reading a complex test file, map the existing tests using:
`grep -E '^\s*#\[(tokio::)?test\]|^\s*fn ' <filepath>`

## 2. Execution Strategy

Stop the noise. Target strictly.

* **Targeted Testing:** NEVER run `cargo test` globally unless explicitly requested. Always target:
`cargo test --package <pkg> --test <test_file> -- <test_name>`
* **Isolate E2E Execution:** E2E tests are slow. They should be run selectively or via CI, not during rapid local iteration unless verifying a specific integration point.

## 3. Architecture, Location, & Intent Distinction

You must distinguish the *intent* of a test strictly by its location and execution boundary. Do not mix mocked integration tests with heavy E2E tests.

| Test Intent | Location | I/O Boundary Constraint |
| --- | --- | --- |
| **Unit** | Bottom of `src/` file inside `mod tests` annotated with `#[cfg(test)]`. | **Strictly Offline.** No I/O. Tests private internals. |
| **Integration** | `tests/integration/<domain>/` directory. | **Strictly Offline.** Black-box public API. Uses injected mocks. |
| **E2E** | `tests/e2e/<domain>/` directory. | **Live I/O Allowed.** Must use `testcontainers` for ephemeral DBs/services. |
| **Fixtures** | `tests/common/` or `test_utils` feature flag. | Shared mock data and container setup code. |

> **Agentic Rule:** When modifying logic in `src/`, you MUST check the `mod tests` block in that exact file to update breaking unit assertions first.

## 4. Strict BDD Naming Convention

Test function names act as documentation and MUST strictly adhere to the Given-When-Then structure using **`snake_case`**.

* **Format:** `given_<state_or_context>_when_<action_taken>_then_<expected_outcome>`
* **The 5-Word Rule:** If a test name is shorter than 5 words, it is too vague and must be expanded.
* **Example:** `#[tokio::test] fn given_running_postgres_container_when_inserting_user_then_persists_to_db() { ... }`

## 5. Mocking vs. Testcontainers

* **Unit & Integration (Hand-Written Stubs):** Prefer lightweight, hand-written, in-memory stubs (e.g., `struct MockRepo`) over heavy macro-based frameworks like `mockall`. They are easier for an LLM to read and modify.
* **E2E (The `testcontainers` Rule):** E2E tests MUST NOT hit shared, persistent, or live remote databases. You **must** use the `testcontainers` crate (or `testcontainers-modules`) to spin up isolated, ephemeral Docker containers for the duration of the test. To prevent port collisions and ensure parallel test execution, always use dynamically assigned host ports (e.g., `.get_host_port_ipv4()`) rather than hardcoding static ports like `5432`.

## 6. Assertions & Safety Exceptions

* **Explicit/Structural Assertions:** Verify intent using structural matching over generic string matching.
* *Use:* `assert!(matches!(result, Err(DomainError::InvalidState)))`
* **No Silent Failures:** Negative tests must actually verify the specific type of error returned, rather than just checking `.is_err()`.
* **The `unwrap()` Exception:** Unlike production code, `.unwrap()` and `.expect()` are ENCOURAGED in test setup to fail fast. Do not waste tokens on complex `Result` boilerplate in test files.
