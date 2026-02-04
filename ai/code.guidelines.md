# Rust Code Guidelines

## Philosophy

Code conveys intent via type signatures, function naming, and module organization.

* Document public APIs: `///`.
* Document non-obvious logic: `// ...`.
* Document safety: `// SAFETY: ...`.

---

## Naming Conventions

### Casing Standards

* **Types:** `PascalCase` (Structs, Enums, Traits).
* **Functions/Variables:** `snake_case`.
* **Constants/Statics:** `SCREAMING_SNAKE_CASE`.
* **Lifetimes/Generics:** Single lowercase or descriptive name.

### Semantic Naming

* **Functions:** Verb phrases (e.g., `calculate_vwap`, `write_to_db`).
* **Types:** Noun phrases (e.g., `VwapAggregator`, `TradeWriter`).
* **Domain:** Use spec-specific terms (e.g., `bps`, `spread`, `vwap`).

---

## Type System Usage

### Newtype Pattern

Use for domain safety to prevent primitive mixing.

```rust
struct BinanceTradeId(u64);
struct KrakenTradeId(u64);

```

### State Machines

Use Enums for explicit state handling.

```rust
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { since: Instant },
}

```

### Type State Pattern

Enforce valid transitions via move semantics.

```rust
impl ConnectionBuilder<Unvalidated> {
    fn validate(self) -> Result<ConnectionBuilder<Validated>> { ... }
}

```

---

## Error Handling

### Library vs Application

* **Library:** Use `thiserror` for defined, variant-based errors.
* **Application:** Use `anyhow` for high-level context and propagation.

### Safety Invariants

* **FORBIDDEN:** `unwrap()` or `expect()` in production hot paths.
* **REQUIRED:** Propagate errors with `?` or map to domain errors.
* **CHANNELS:** Explicitly handle `RecvError::Lagged` and `RecvError::Closed`.

---

## Async & Concurrency

### Patterns

* **Observability:** Name `tokio::spawn` handles.
* **Concurrency:** Use `tokio::select!` for multi-signal loops.
* **Runtime Safety:** Never use blocking `std::thread::sleep` in async context; use `tokio::time::sleep`.

### Ownership

* **Shared Read:** Use `Arc<T>`.
* **Shared Mutable:** Use `Arc<RwLock<T>>`.
* **Resource Guards:** Consume `self` in builder `run()` methods to prevent reuse.

---

## Performance

### Optimization Rules

* **Allocation:** Prefer `&str`/`&[T]` over `String`/`Vec<T>` in parameters.
* **Pre-allocation:** Use `Vec::with_capacity` when size is known.
* **Hot Paths:** Avoid `.clone()`. Iterate via `.iter()` to borrow.

---

## Module Organization

### Structure

* **Granularity:** One concern per module.
* **Re-exports:** Use `mod.rs` or `lib.rs` for `pub use`.
* **Hierarchy:** Prefer flat structures over deep nesting.

### Traits

* **Definition:** Place in `core` crates.
* **Implementation:** Place in `infrastructure` or outer layers.
* **Dispatch:** Prefer static dispatch (`T: Trait`) over dynamic dispatch (`&dyn Trait`).

---

## Testing & CI

### Conventions

* **Unit Tests:** Internal `mod tests` with `#[cfg(test)]`.
* **Integration:** Separate `tests/` directory.
* **Linting:** Required `cargo clippy` and `cargo fmt`.

### Workspace Management

* **Dependencies:** Centralize in root `Cargo.toml` `[workspace.dependencies]`.
* **Features:** Opt-in to specific crate features; avoid `full` versions to minimize bloat.

---

## Anti-Patterns

* **God Structs:** Violates SRP.
* **Stringly-typed APIs:** Replace with Enums/Newtypes.
* **Silent Failures:** Use `_` only when intentional; log or propagate otherwise.
* **Mutex Overuse:** Use `RwLock` or channels for better concurrency.