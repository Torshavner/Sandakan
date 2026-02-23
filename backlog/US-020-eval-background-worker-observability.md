# Eval Background Worker with Feature Flag + Outbox Pattern

## Status: IMPLEMENTED

## Requirement Definition

As a **RAG System Developer**, I need **an opt-in background evaluation worker backed by the Outbox Pattern** so that **eval scoring is durable across restarts and decoupled from RAG query latency**.

---

## Context

The eval harness (US-009) is complete: passive `EvalEvent` capture → `PgEvalEventRepository` → CLI `evaluate` binary. This story adds:

1. **Feature Flag** — eval is opt-in (`eval.enabled = false` by default). When disabled, no events are recorded and no worker runs.
2. **Outbox Pattern** — replace the current fire-and-forget `tokio::spawn` in `RetrievalService::query()` with a durable `eval_outbox` Postgres table. A dedicated background worker polls and processes outbox rows using `FOR UPDATE SKIP LOCKED`.

Eval results are emitted as structured `tracing::info!` events. A **separate story** will add the observability infrastructure (Loki, Vector, Tempo, Grafana, Docker Compose) to consume these events.

---

## Dependency: US-017 (Broker Abstraction) Compatibility

`EvalOutboxEntry` has `Serialize + Deserialize` derives (US-017 broker bounds). `EvalWorker` separates `receive_batch()` (transport concern — outbox polling) from `process_entry()` (business logic). When US-017 lands, `receive_batch()` extracts into `OutboxSubscriber<EvalOutboxEntry>::receive()` — mechanical refactor.

---

## Architecture

```
RAG query path (no latency impact):
  RetrievalService::query()
    ├─ if eval_enabled → INSERT eval_event + INSERT eval_outbox (pending) — fire-and-forget spawn
    └─ return QueryResponse immediately

Background EvalWorker (spawned at startup if eval_enabled):
  tokio::spawn(EvalWorker::run())
    └─ every N seconds: claim_pending(batch_size) with FOR UPDATE SKIP LOCKED
         └─ for each row:
              load EvalEvent → run faithfulness metric
              → tracing::info!(faithfulness, failed, model_config, "eval.result")
              → mark outbox row done/failed
```

---

## Layer Responsibilities (Hexagonal, maintained)

| Layer | Change |
|---|---|
| **L1 Domain** | `EvalOutboxEntry`, `EvalOutboxStatus` |
| **L2 Ports** | `EvalOutboxRepository` trait |
| **L2 Services** | `EvalWorker`; modified `RetrievalService` (Option fields for eval repos) |
| **L3 Infrastructure** | `PgEvalOutboxRepository`; migration `004_create_eval_outbox_table.sql` |
| **L4 Presentation** | `EvalSettings` (`enabled`, `worker_poll_interval_secs`, `worker_batch_size`); `main.rs` conditional wiring |

---

## Deferred

The following will be addressed in a **separate observability story**:
- Docker Compose overlay (Loki, Vector, Tempo, Grafana)
- Vector log shipper configuration
- Loki configuration
- Grafana dashboard provisioning
- Tempo distributed tracing

The structured `tracing::info!` events emitted by `EvalWorker` are the contract surface — the observability stack consumes them without code changes.
