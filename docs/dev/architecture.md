# Architecture

Ork is a workflow orchestrator built from a small set of crates. This page uses concrete Ork names so the diagrams map directly to code.

## Components (Ork names)

```mermaid
graph TB
    subgraph CLI["crates/ork-cli"]
        OrkCLI["ork (src/main.rs)"]
    end

    subgraph Core["crates/ork-core"]
        Scheduler["Scheduler<D,E>"]
        Models["models::{Workflow, Run, Task}"]
    end

    subgraph State["crates/ork-state"]
        DB["PostgresDatabase / FileDatabase"]
    end

    subgraph Executors["crates/ork-executors"]
        ExecMgr["ExecutorManager"]
        ProcExec["ProcessExecutor"]
        CloudExec["CloudRunClient"]
    end

    subgraph Workers["Workers"]
        LocalJob["local script: {script_dir}/{job_name}"]
        CloudJob["Cloud Run execution"]
    end

    OrkCLI --> Scheduler
    Scheduler <--> DB
    Scheduler --> ExecMgr
    ExecMgr --> ProcExec
    ExecMgr --> CloudExec
    ProcExec -->|spawn| LocalJob
    CloudExec -->|create + poll| CloudJob
    ProcExec -. "StatusUpdate via mpsc::unbounded_channel" .-> Scheduler
    CloudExec -. "StatusUpdate via mpsc::unbounded_channel" .-> Scheduler
```

## Component Responsibilities

| Component | Role |
|-----------|------|
| [ork-cli (ork)](crates/ork-cli.md) | Starts the scheduler loop, exposes CLI commands, and hosts perf-test tooling | 
| [ork-core::Scheduler](../../crates/ork-core/src/scheduler.rs) | Main loop: finds pending runs/tasks, dispatches work, processes StatusUpdates | 
| [ork-core::Database (trait)](../../crates/ork-core/src/database.rs) | Storage contract for runs/tasks/workflows used by Scheduler | 
| [ork-state::PostgresDatabase](../../crates/ork-state/src/postgres.rs) | Default Database implementation for production Postgres | 
| [ork-state::FileDatabase](../../crates/ork-state/src/file_database.rs) | File-backed Database for local/dev use | 
| [ork-executors::ExecutorManager](../../crates/ork-executors/src/manager.rs) | Maps workflow_id -> executor instance and wires status channels | 
| [ork-executors::ProcessExecutor](../../crates/ork-executors/src/process.rs) | Executes local scripts, reports status via StatusUpdate | 
| [ork-executors::CloudRunClient](../../crates/ork-executors/src/cloud_run.rs) | Creates and polls Cloud Run executions, reports status | 
| [ork-runner::LocalScheduler (legacy)](../../crates/ork-runner/src/scheduler.rs) | Old scheduler used by ork-web; pending migration | 
| [ork-web (legacy)](crates/ork-web.md) | Axum UI/API built on ork-runner | 

## Data Flow

### Run Creation Flow

```mermaid
sequenceDiagram
    participant OrkCLI as ork-cli (ork)
    participant DB as ork-state::PostgresDatabase
    participant Scheduler as ork-core::Scheduler
    participant ExecMgr as ork-executors::ExecutorManager

    OrkCLI->>DB: create_workflow(...)
    OrkCLI->>DB: create_run(...)
    OrkCLI->>Scheduler: Scheduler::run() (long-running)

    Scheduler->>DB: get_pending_runs()
    DB-->>Scheduler: [Run]
    Scheduler->>DB: get_workflows_by_ids(...)
    Scheduler->>ExecMgr: register_workflow(workflow)
    Scheduler->>DB: batch_create_tasks(run_id, task_count)
    Scheduler->>DB: update_run_status(running)
```

### Task Execution Flow (Event-Driven)

```mermaid
sequenceDiagram
    participant Scheduler as ork-core::Scheduler
    participant DB as ork-state::PostgresDatabase
    participant ExecMgr as ork-executors::ExecutorManager
    participant Exec as ork-executors::ProcessExecutor / CloudRunClient
    participant Worker as Worker (script or Cloud Run execution)
    participant Channel as StatusUpdate channel

    loop poll_interval_secs
        Scheduler->>DB: get_pending_tasks_with_workflow(limit)
        DB-->>Scheduler: [TaskWithWorkflow]
    end

    Scheduler->>ExecMgr: get_executor(workflow_id)
    ExecMgr-->>Scheduler: Arc<Executor>
    Scheduler->>Exec: execute(task_id, job_name, params)
    Exec->>Worker: spawn / create execution
    Scheduler->>DB: batch_update_task_status(dispatched | failed)

    Worker-->>Exec: exit / completion status
    Exec->>Channel: StatusUpdate{task_id, status}
    Channel-->>Scheduler: StatusUpdate

    Scheduler->>DB: batch_update_task_status(running | success | failed)
    Scheduler->>DB: get_run_task_stats(run_id)
    alt run complete
        Scheduler->>DB: update_run_status(success | failed)
    end
```

## Key Design Decisions (mapped to code)

- **Event-driven updates**: [`ork-core::Scheduler`](../../crates/ork-core/src/scheduler.rs) listens on an `mpsc::unbounded_channel` for `StatusUpdate` events from executors.
- **Batch DB ops**: [`ork-state::PostgresDatabase`](../../crates/ork-state/src/postgres.rs) implements batch create/update to reduce round trips.
- **Bounded concurrency**: dispatch uses `buffer_unordered` controlled by [`OrchestratorConfig`](../../crates/ork-core/src/config.rs).
- **Executor isolation**: [`ork-executors::ExecutorManager`](../../crates/ork-executors/src/manager.rs) chooses backend per workflow.

## Current Limitations

- No retry logic (tasks fail permanently)
- No timeout handling (tasks can run indefinitely)
- No heartbeat monitoring (hung tasks are not detected)
- No DAG support (tasks are independent)
- No scheduled runs (manual trigger only)
