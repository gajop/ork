# Database Schema

Ork uses a relational database (PostgreSQL or SQLite) for all state management. No object store or external state required.

## Tables

### Workflows

Workflow definitions with executor configuration.

```sql
CREATE TABLE workflows (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    job_name TEXT NOT NULL,              -- Script/job to execute
    region TEXT NOT NULL,                 -- Executor region (e.g., 'us-central1', 'local')
    project TEXT NOT NULL,                -- GCP project or 'local'
    executor_type TEXT NOT NULL,          -- 'cloudrun' or 'process'
    task_params JSONB,                    -- Optional parameters (e.g., task_count)
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);
```

**Example row:**
```json
{
  "id": "d584f91d-7dc3-4637-8d24-ea732e303898",
  "name": "example-process",
  "description": null,
  "job_name": "example-task.sh",
  "region": "local",
  "project": "local",
  "executor_type": "process",
  "task_params": {"task_count": 5},
  "created_at": "2024-01-30T12:00:00Z",
  "updated_at": "2024-01-30T12:00:00Z"
}
```

### Runs

Workflow execution instances.

```sql
CREATE TABLE runs (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'success', 'failed', 'cancelled')),
    triggered_by TEXT NOT NULL DEFAULT 'manual',
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_workflow_id ON runs(workflow_id);
```

**Status flow:** `pending` → `running` → `success`/`failed`

**Example row:**
```json
{
  "id": "f0888034-0abe-4d9c-af81-1137dff6c359",
  "workflow_id": "d584f91d-7dc3-4637-8d24-ea732e303898",
  "status": "success",
  "triggered_by": "manual",
  "started_at": "2024-01-30T23:50:31Z",
  "finished_at": "2024-01-30T23:50:34Z",
  "error": null,
  "created_at": "2024-01-30T23:50:31Z"
}
```

### Tasks

Individual task executions within a run.

```sql
CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    task_index INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'dispatched', 'running', 'success', 'failed', 'cancelled')),
    execution_name TEXT,                  -- Process ID or Cloud Run execution name
    params JSONB,
    output JSONB,
    error TEXT,
    dispatched_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE(run_id, task_index)
);

CREATE INDEX idx_tasks_run_id ON tasks(run_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_status_run_id ON tasks(status, run_id)
    WHERE status IN ('pending', 'dispatched', 'running');
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);
CREATE INDEX idx_tasks_execution_name ON tasks(execution_name)
    WHERE execution_name IS NOT NULL;
```

**Status flow:** `pending` → `dispatched` → `running` → `success`/`failed`

**Example row:**
```json
{
  "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
  "run_id": "f0888034-0abe-4d9c-af81-1137dff6c359",
  "task_index": 0,
  "status": "success",
  "execution_name": "12345",
  "params": null,
  "output": null,
  "error": null,
  "dispatched_at": "2024-01-30T23:50:31Z",
  "started_at": "2024-01-30T23:50:31Z",
  "finished_at": "2024-01-30T23:50:31Z",
  "created_at": "2024-01-30T23:50:31Z"
}
```

## Performance Indexes

### Composite Indexes

Critical for query performance:

```sql
-- Finding pending tasks with run info (JOIN query)
CREATE INDEX idx_tasks_status_run_id ON tasks(status, run_id)
    WHERE status IN ('pending', 'dispatched', 'running');

-- Checking run completion
CREATE INDEX idx_tasks_run_status ON tasks(run_id, status);
```

These enable the scheduler to:
1. Fetch pending tasks + workflow in single JOIN query (not N+1)
2. Check if all tasks in a run are complete with a single COUNT query

### Executor Type Index

```sql
CREATE INDEX idx_workflows_executor_type ON workflows(executor_type);
```

Allows filtering workflows by executor (future use).

### Execution Name Index

```sql
CREATE INDEX idx_tasks_execution_name ON tasks(execution_name)
    WHERE execution_name IS NOT NULL;
```

Used by Cloud Run executor to look up tasks by execution name during status polling.

## Database Trait Implementations

### PostgresDatabase

- Uses sqlx connection pool
- Transactions for batch operations
- Prepared statements for all queries
- Connection pooling with max 10 connections

### FileDatabase (SQLite-based)

- JSON files in directory structure:
  - `workflows/{workflow_id}.json`
  - `runs/{run_id}.json`
  - `tasks/{task_id}.json`
- Uses serde_json for serialization
- No indexes (file-based, not query-optimized)
- Suitable for development/testing

## Migrations

Located in `crates/ork-cli/migrations/`:

1. **001_initial.sql** - Create workflows, runs, tasks tables
2. **002_add_executor_type.sql** - Add executor_type column
3. **003_performance_indexes.sql** - Add composite indexes
4. **004_rename_cloud_run_columns.sql** - Rename to executor-agnostic names

Applied via `sqlx migrate run` (embedded in binary).
