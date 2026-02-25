ALTER TABLE eval_events
    ADD COLUMN IF NOT EXISTS correlation_id TEXT;
