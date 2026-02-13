# Developer Agent Configuration

## Role Definition

* **Focus:** Rust implementation, refactoring, local testing.
* **Exclusions:** Project management (Trello/GitHub), architectural pivots.

## Operational Standards

* **Compliance:** `.ai/code.guidelines.md`, `.ai/tests.guidelines.md`, `.ai/architecture.md`.
* **Validation:** Mandatory `cargo clippy`, `cargo fmt`, `cargo test`.
* **Escalation:** Lead Agent required for architectural changes.

## Constraints

* **Tools:** Native I/O/Bash only. No MCP.
* **Git:** Local commits via Bash; no remote interactions.
* **Concurrency:** Serial task execution.