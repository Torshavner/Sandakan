# Code Guidelines

## 1. AI Context Navigation & Macro-Routing

To conserve context window and ensure deterministic navigation, this repository acts as a searchable graph. Top-level directories represent **business domains**, not technical layers.

* **The `wc -l` Gateway:** Large files destroy LLM attention. Before reading any implementation file, you MUST run `wc -l <filepath>`.
* **The 250-Line Rule & The Bypass:** If a file exceeds 250 lines, it generally violates SRP.

1. **Exception:** If the file begins with the `// @AI-BYPASS-LENGTH` header, **ignore the 250-line limit** and proceed normally. (Reserve this for complex configurations or un-splittable match blocks).
2. **If no bypass exists, DO NOT** read the full file.
3. Map symbols using `grep -E '^(pub )?(struct|enum|fn|impl|trait)' <filepath>`.
4. Propose extracting complex `impl` blocks or enums into private submodules before proceeding.

## 2. Clean Architecture & HTTP Handlers

* **Zero-DTO Handlers:** Handlers MUST contain only orchestration (`Extract -> Service -> Map`).
* **Strict Isolation:** Define DTOs, Requests, and Responses in a dedicated `schema.rs` or `contract` crate. Never define them inside handler files.
* **Trait Boundaries:** Decouple handlers from infrastructure using traits defined in `core` crates. Implement them in `infrastructure`. Prefer static dispatch (`T: Trait`) over dynamic (`&dyn Trait`) to enable fast AI mocking without reading DB implementations.

## 3. Naming Conventions, Type System & Commenting Strategy

Code conveys intent via signatures and types, but context often requires plain English. We do not ban comments; instead, we strictly categorize them by their purpose.

### Commenting Taxonomy & Rules

* **The "Why", Not the "What" (`//`):** Inline comments are strictly reserved for explaining business constraints, domain edge-cases, or acknowledging a hack/workaround due to an upstream bug. Never translate Rust syntax into English. If you need a comment to explain what a block of code is doing, the code must be refactored or extracted into a well-named function.
* **Documentation Comments (`///`, `//!`):** Use these to define the contract of public APIs, structs, and traits. As stated in Section 1, module-level documentation (`/// @AI:`) is strictly mandatory for domain routing.
* **Mandatory Pragmatic Comments (`// SAFETY:`, `// @AI-BYPASS-LENGTH:`, `// TODO:`):** Strictly required above `unsafe` blocks to explain invariants, required to bypass the 250-line limit, and acceptable for tracking technical debt (provided they include context and a ticket number).
* **Zero-Tolerance Anti-Patterns:** Do not leave commented-out code (delete it; Git preserves history) and do not write redundant variable explanations (let the type system do the talking).

### Naming & Type System

* **Casing:** Use `PascalCase` for Types, `snake_case` for Functions/Variables, and `SCREAMING_SNAKE_CASE` for Constants.
* **Semantics:** Use verb phrases for functions (e.g., `calculate_vwap`) and noun phrases for types (e.g., `TradeWriter`). Strictly use exact terminology from the domain specification.
* **Newtype Pattern:** Wrap primitives to enforce domain safety and prevent "stringly-typed" or "primitive-obsessed" APIs (e.g., `struct UserId(u64);`, `struct EmailAddress(String);`).
* **State Machines & Typestates:** Use `enum` for explicit state handling. Enforce valid state transitions at compile-time via move semantics and generics (e.g., a function that takes `ConnectionBuilder<Unvalidated>` and returns `ConnectionBuilder<Validated>`).

## 4. Async & Concurrency

* **Runtime Rules:** Never use `std::thread::sleep` in async contexts; use `tokio::time::sleep`. Use `tokio::select!` for multi-signal loops.
* **Observability:** Always name your `tokio::spawn` handles.
* **Ownership & Locks:** Use `Arc<T>` for shared reads and `Arc<RwLock<T>>` for shared mutation. Do not overuse `Mutex`; prefer `RwLock` or channels.
* **Resource Guards:** Consume `self` in builder `run()` methods to prevent reuse.

## 5. Error Handling & Safety

* **Crates:** Use `thiserror` for defined, variant-based library errors. Use `anyhow` for app-level context propagation.
* **Safety Invariants:** `unwrap()` and `expect()` are **FORBIDDEN** in production hot paths. Map to domain errors or propagate with `?`. Require `// SAFETY:` comments for any `unsafe` block.
* **Channels:** Explicitly handle `RecvError::Lagged` and `RecvError::Closed`.
* **Silent Failures:** Use `_` only when intentional; otherwise, log or propagate.

## 6. Performance

* **Borrowing:** Prefer `&str`/`&[T]` over `String`/`Vec<T>` in function parameters.
* **Pre-allocation:** Always use `Vec::with_capacity` when the size is known.
* **Hot Paths:** Avoid `.clone()`. Iterate via `.iter()` to borrow data.

## 7. Testing, CI & Workspace

* **Testing Organization:** Place unit tests in an internal `mod tests` with `#[cfg(test)]` at the bottom of the file. Use a separate `tests/` directory for integration tests.
* **Workspace Management:** Centralize dependency versions in the root `Cargo.toml` under `[workspace.dependencies]`. Opt-in to specific crate features; avoid `full` flags to minimize bloat.
* **Linting:** `cargo clippy` and `cargo fmt` are strictly mandatory.
