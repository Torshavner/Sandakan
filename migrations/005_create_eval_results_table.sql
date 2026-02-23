CREATE TABLE IF NOT EXISTS eval_results (
    id               UUID PRIMARY KEY,
    eval_event_id    UUID NOT NULL REFERENCES eval_events(id) ON DELETE CASCADE,
    faithfulness     REAL NOT NULL,
    context_recall   REAL,
    correctness      REAL,
    below_threshold  BOOLEAN NOT NULL,
    computed_at      TIMESTAMPTZ NOT NULL,
    UNIQUE (eval_event_id)
);

-- Grafana time-series queries filter by computed_at
CREATE INDEX idx_eval_results_computed_at ON eval_results(computed_at DESC);

-- Fast filtering for dashboards showing only failing evaluations
CREATE INDEX idx_eval_results_below_threshold ON eval_results(below_threshold)
    WHERE below_threshold = true;
