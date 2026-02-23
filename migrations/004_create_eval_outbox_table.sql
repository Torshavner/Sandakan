CREATE TABLE IF NOT EXISTS eval_outbox (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    eval_event_id  UUID NOT NULL REFERENCES eval_events(id) ON DELETE CASCADE,
    status         TEXT NOT NULL DEFAULT 'pending',
    error          TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partial index for fast pending polling by the EvalWorker
CREATE INDEX idx_eval_outbox_pending ON eval_outbox(created_at)
    WHERE status = 'pending';
