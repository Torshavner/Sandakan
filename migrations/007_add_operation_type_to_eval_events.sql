ALTER TABLE eval_events
    ADD COLUMN operation_type TEXT NOT NULL DEFAULT 'query';

CREATE INDEX idx_eval_events_operation_type ON eval_events(operation_type);
