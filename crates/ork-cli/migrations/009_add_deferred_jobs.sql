-- Add deferred_jobs table for tracking long-running external jobs
-- Used by the Triggerer component to poll external APIs (BigQuery, Cloud Run, etc.)

CREATE TABLE IF NOT EXISTS deferred_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,

    -- Job tracking info
    service_type TEXT NOT NULL, -- 'bigquery', 'cloudrun', 'dataproc', 'custom_http'
    job_id TEXT NOT NULL,       -- External job identifier
    job_data JSONB NOT NULL,    -- Full deferrable JSON (project, location, etc.)

    -- Status tracking
    status TEXT NOT NULL DEFAULT 'pending',
    error TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    last_polled_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT deferred_jobs_status_check CHECK (
        status IN ('pending', 'polling', 'completed', 'failed', 'cancelled')
    )
);

-- Indexes for efficient queries
CREATE INDEX idx_deferred_jobs_task_id ON deferred_jobs(task_id);
CREATE INDEX idx_deferred_jobs_status ON deferred_jobs(status);
CREATE INDEX idx_deferred_jobs_service_type ON deferred_jobs(service_type);
CREATE INDEX idx_deferred_jobs_status_service ON deferred_jobs(status, service_type)
    WHERE status IN ('pending', 'polling');

-- Index for finding jobs to poll (not finished yet)
CREATE INDEX idx_deferred_jobs_polling ON deferred_jobs(last_polled_at)
    WHERE status IN ('pending', 'polling');
