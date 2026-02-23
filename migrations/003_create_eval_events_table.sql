CREATE TABLE IF NOT EXISTS eval_events (
    id               UUID PRIMARY KEY,
    timestamp        TIMESTAMPTZ NOT NULL,
    question         TEXT NOT NULL,
    generated_answer TEXT NOT NULL,
    retrieved_sources JSONB NOT NULL DEFAULT '[]',
    model_config     TEXT NOT NULL
);

CREATE INDEX idx_eval_events_timestamp ON eval_events(timestamp DESC);
CREATE INDEX idx_eval_events_model_config ON eval_events(model_config);
