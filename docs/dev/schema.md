Status: Pending Review

# Schema

## State Store

### Workflows Collection

```
workflows/{workflow_id}
```

```json
{
  "id": "uuid",
  "name": "my_etl",
  "definition": {
    "tasks": [
      {
        "name": "extract",
        "executor": "python",
        "file": "tasks/extract.py",
        "depends_on": [],
        "timeout_seconds": 600,
        "retries": 0,
        "resources": {"cpu": 1000, "memory_mb": 512}
      }
    ]
  },
  "schedule": "0 2 * * *",
  "version": "abc123",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}
```

### Runs Collection

```
runs/{run_id}
```

```json
{
  "id": "uuid",
  "workflow_id": "uuid",
  "workflow_name": "my_etl",
  "workflow_version": "abc123",
  "status": "running",
  "triggered_by": "schedule",
  "input": {},
  "owner": "scheduler-abc",
  "lease_expires": "2024-01-01T00:01:00Z",
  "started_at": "2024-01-01T00:00:00Z",
  "finished_at": null,
  "created_at": "2024-01-01T00:00:00Z"
}
```

Status values: `pending`, `running`, `success`, `failed`, `cancelled`

### Task Runs Collection

```
runs/{run_id}/tasks/{task_id}
```

```json
{
  "id": "uuid",
  "run_id": "uuid",
  "task_name": "extract",
  "status": "running",
  "attempt": 1,
  "max_retries": 0,
  "parallel_index": null,
  "parallel_count": null,
  "executor_id": "job-xyz",
  "input": {},
  "output": null,
  "error": null,
  "dispatched_at": "2024-01-01T00:00:05Z",
  "started_at": "2024-01-01T00:00:10Z",
  "finished_at": null,
  "created_at": "2024-01-01T00:00:00Z"
}
```

Status values: `pending`, `dispatched`, `running`, `success`, `failed`, `skipped`

### Schedules Collection

```
schedules/{schedule_id}
```

```json
{
  "id": "uuid",
  "workflow_id": "uuid",
  "cron_expr": "0 2 * * *",
  "next_run": "2024-01-02T02:00:00Z",
  "enabled": true,
  "created_at": "2024-01-01T00:00:00Z"
}
```

## Object Store

```
{bucket}/
└── runs/
    └── {run_id}/
        └── {task_id}/
            ├── spec.json
            ├── status.json
            └── output.json
```

### spec.json

Written by scheduler before dispatching.

```json
{
  "task_id": "uuid",
  "run_id": "uuid",
  "task_name": "extract",
  "executor": "python",
  "file": "tasks/extract.py",
  "input": {
    "api_url": "https://api.example.com"
  },
  "upstream": {
    "other_task": {
      "field": "value"
    }
  },
  "timeout_seconds": 600,
  "parallel_index": null,
  "parallel_count": null,
  "attempt": 1,
  "created_at": "2024-01-01T00:00:00Z"
}
```

### status.json

Written by worker, polled by scheduler.

```json
{
  "status": "running",
  "heartbeat": "2024-01-01T00:05:30Z",
  "started_at": "2024-01-01T00:05:00Z",
  "error": null
}
```

Status values: `running`, `success`, `failed`

### output.json

Written by worker on success.

```json
{
  "users": [
    {"id": 1, "name": "Alice", "email": "alice@example.com"},
    {"id": 2, "name": "Bob", "email": "bob@example.com"}
  ]
}
```

## Postgres Schema

For self-hosted deployments using Postgres:

```sql
CREATE TABLE workflows (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    definition JSONB NOT NULL,
    schedule TEXT,
    version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    workflow_name TEXT NOT NULL,
    workflow_version TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    triggered_by TEXT NOT NULL,
    input JSONB NOT NULL DEFAULT '{}',
    owner TEXT,
    lease_expires TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE task_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    task_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempt INT NOT NULL DEFAULT 0,
    max_retries INT NOT NULL DEFAULT 0,
    parallel_index INT,
    parallel_count INT,
    executor_id TEXT,
    input JSONB NOT NULL DEFAULT '{}',
    output JSONB,
    error TEXT,
    dispatched_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(run_id, task_name, parallel_index)
);

CREATE TABLE schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    cron_expr TEXT NOT NULL,
    next_run TIMESTAMPTZ NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_lease ON runs(status, lease_expires) WHERE status = 'running';
CREATE INDEX idx_task_runs_status ON task_runs(run_id, status);
CREATE INDEX idx_schedules_next_run ON schedules(next_run) WHERE enabled = true;
```
