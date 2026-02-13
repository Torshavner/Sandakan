CREATE TABLE IF NOT EXISTS jobs (
    id            UUID PRIMARY KEY,
    document_id   UUID,
    status        TEXT NOT NULL DEFAULT 'QUEUED',
    job_type      TEXT NOT NULL,
    error_message TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jobs_status ON jobs(status);
CREATE INDEX idx_jobs_document_id ON jobs(document_id);
