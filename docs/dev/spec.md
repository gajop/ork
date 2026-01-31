# Technical Specification

This document captures the stable behavior of Ork at a high level. For authoritative details, see the referenced files.

## State Machines

### Run Lifecycle (DB status)

```
[*] --> pending: workflow triggered
pending --> running: Scheduler creates tasks
running --> success: all tasks succeeded
running --> failed: any task failed
running --> cancelled: user cancels (reserved)
success --> [*]
failed --> [*]
cancelled --> [*]
```

**Transitions:**
- `pending` -> `running`: [`ork-core::Scheduler`](../../crates/ork-core/src/scheduler.rs) creates tasks and updates the run status.
- `running` -> `success`: all tasks complete successfully.
- `running` -> `failed`: any task fails.
- `cancelled` exists in the schema but is not wired in yet.

### Task Lifecycle (DB status)

```
[*] --> pending: run enters running
pending --> dispatched: Scheduler dispatches to executor

dispatched --> running: executor reports started
running --> success: executor reports completion
running --> failed: executor reports failure

success --> [*]
failed --> [*]
cancelled --> [*]
```

**Transitions:**
- `pending` -> `dispatched`: Scheduler calls [`Executor::execute`](../../crates/ork-core/src/executor.rs) and persists the execution name.
- `dispatched` -> `running`: executor sends [`StatusUpdate`](../../crates/ork-core/src/executor.rs) `{ status: "running" }`.
- `running` -> `success` / `failed`: executor sends terminal [`StatusUpdate`](../../crates/ork-core/src/executor.rs).
- `cancelled` exists in the schema but is not wired in yet.

## Scheduler Behavior (summary)

- Loop lives in [crates/ork-core/src/scheduler.rs](../../crates/ork-core/src/scheduler.rs).
- Each tick:
  - Fetches pending runs, loads workflows + `workflow_tasks`, creates tasks, then marks runs `running`.
  - Fetches pending tasks with workflow info, dispatches concurrently, marks tasks `dispatched` or `failed`.
  - Drains `StatusUpdate` channel and applies task status updates, then checks run completion.
- Executor selection uses `Task.executor_type` with per-task params for job/command overrides.
- For workflows with `workflow_tasks` (compiled DAGs), pending tasks are only dispatched after all dependencies succeed; downstream tasks are failed if a dependency fails.
- When idle, the scheduler sleeps until either the poll interval elapses or a status update arrives.
- Metrics are logged per loop iteration.

## Executor Behavior (summary)

- [crates/ork-executors/src/process.rs](../../crates/ork-executors/src/process.rs):
  - Spawns local scripts (`{script_dir}/{command}`) or Python tasks via `scripts/run_python_task.py`.
- [crates/ork-executors/src/cloud_run.rs](../../crates/ork-executors/src/cloud_run.rs):
  - Creates Cloud Run executions and polls until terminal, then reports status.
- Executors send updates via the [`StatusUpdate`](../../crates/ork-core/src/executor.rs) channel set by the scheduler.

## Storage Notes

- Canonical schema lives in [crates/ork-cli/schema.sql](../../crates/ork-cli/schema.sql) and [crates/ork-cli/migrations/](../../crates/ork-cli/migrations/).
- Compiled DAGs are stored in `workflow_tasks` (see [crates/ork-cli/schema.sql](../../crates/ork-cli/schema.sql)).
- [`PostgresDatabase`](../../crates/ork-state/src/postgres.rs) implements batch operations used by the scheduler.
- [`FileDatabase`](../../crates/ork-state/src/file_database.rs) is intended for local/dev workflows and does not implement all optimizations.

## Current Limitations

- No retries or timeouts
- No heartbeats or stuck-task detection
- No output passing between tasks (ordering-only DAGs)
- Single scheduler instance (no leasing / HA)
