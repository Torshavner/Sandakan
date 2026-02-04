# Rust Code Guidelines

## Philosophy

Code conveys intent via signatures and structure.

* **Public API:** `///` mandatory.
* **Logic:** Explain *why*, not *how*.
* **Safety:** `// SAFETY:` invariants required.

---

## Architecture & File Structure

**Screaming Architecture:** Directory structure reveals domain intent, not framework patterns.

* **One Type, One File:** STRICT. Isolate every major `struct`, `enum`, or `trait` into its own `.rs` file.
* **Naming:** File name must match type name in `snake_case`.
* `struct OrderBook` → `order_book.rs`
* `enum TradeSide` → `trade_side.rs`


* **Grouping:** Organize by domain component (Feature-First), not technical layer.
* *Good:* `src/matching_engine/`, `src/risk_check/`
* *Bad:* `src/models/`, `src/utils/`


* **Visibility:** Use `mod.rs` to expose public API; hide implementation details.

---

## Naming Conventions

### Casing

* **Types:** `PascalCase` (`Struct`, `Enum`).
* **Functions/Vars:** `snake_case`.
* **Constants:** `SCREAMING_SNAKE`.
* **Generics:** `T` or descriptive `State`.

### Semantic

* **Functions:** Verb-first (`calculate_vwap`).
* **Types:** Noun-first (`VwapAggregator`).
* **Domain:** Strict spec terminology (`bps`, `spread`).

---

## Type System Usage

### Patterns

* **Newtype:** `struct TradeId(u64)` to prevent primitive mixing.
* **State Machines:** `enum` for mutually exclusive states.
* **Type State:** Enforce transitions via move semantics (`fn validate(self) -> Validated`).

---

## Error Handling

### Strategy

* **Library:** `thiserror` for structural errors.
* **App:** `anyhow` for context propagation.
* **Constraints:**
* `unwrap()`/`expect()` **FORBIDDEN** in hot paths.
* Handle `RecvError` explicitly in channels.



---

## Async & Concurrency

### Patterns

* **Runtime:** No blocking I/O (`std::thread::sleep`). Use `tokio`.
* **Concurrency:** `tokio::select!` for loops.
* **Observability:** Name spawned tasks.
* **Ownership:** `Arc<T>` (Read), `Arc<RwLock<T>>` (Write).

---

## Performance

### Optimization

* **Allocations:** `&str` over `String` in args.
* **Capacity:** `Vec::with_capacity` mandatory.
* **Hot Paths:** Zero `clone()`, use `iter()` borrows.

---

## Testing & CI

### Standards

* **Unit:** Colocated `#[cfg(test)] mod tests`.
* **Integration:** `tests/` directory.
* **Tooling:** `cargo clippy`, `cargo fmt`.
* **Deps:** Workspace-level versioning.

---

## Anti-Patterns

* **God Files:** Violates "One Type, One File" rule.
* **Stringly-Typed:** Use Enums.
* **Silent Failures:** `let _ =` forbidden without logs.
* **Mutex Abuse:** Prefer `RwLock` or Channels.