# Architecture

Ork is a workflow orchestrator with DAGs compiled up front and stored in the database. This page uses concrete Ork names so the diagrams map directly to code.

## Components (Ork names)

```mermaid
graph TB
    subgraph CLI["crates/ork-cli"]
        OrkCLI["ork (src/main.rs)"]
    end

    subgraph Core["crates/ork-core"]
        Scheduler["Scheduler<D,E>"]
        Compiler["workflow::Workflow + compiled::CompiledWorkflow"]
    end

    subgraph State["crates/ork-state"]
        DB["PostgresDatabase / FileDatabase"]
    end

    subgraph Tables["DB tables"]
        Workflows["workflows"]
        WorkflowTasks["workflow_tasks"]
        Runs["runs"]
        Tasks["tasks"]
    end

    subgraph Executors["crates/ork-executors"]
        ExecMgr["ExecutorManager"]
        ProcExec["ProcessExecutor"]
        CloudExec["CloudRunClient"]
    end

    subgraph Workers["Workers"]
        LocalJob["local script: {script_dir}/{command}"]
        PyTask["python task: scripts/run_python_task.py + {task_file}"]
        CloudJob["Cloud Run execution"]
    end

    OrkCLI -->|create-workflow-yaml| Compiler
    Compiler --> DB
    DB --> Workflows
    DB --> WorkflowTasks
    OrkCLI --> Scheduler
    Scheduler <--> DB
    DB --> Runs
    DB --> Tasks
    Scheduler --> ExecMgr
    ExecMgr --> ProcExec
    ExecMgr --> CloudExec
    ProcExec -->|spawn| LocalJob
    ProcExec -->|python| PyTask
    CloudExec -->|create + poll| CloudJob
    ProcExec -. "StatusUpdate via mpsc::unbounded_channel" .-> Scheduler
    CloudExec -. "StatusUpdate via mpsc::unbounded_channel" .-> Scheduler
```

## Component Responsibilities

| Component | Role |
|-----------|------|
| [ork-cli (ork)](crates/ork-cli.md) | Defines workflows, compiles YAML DAGs, starts the scheduler loop |
| [ork-core::Scheduler](../../crates/ork-core/src/scheduler.rs) | Main loop: finds pending runs/tasks, dispatches work, processes `StatusUpdate`s |
| [ork-core::workflow + compiled](../../crates/ork-core/src/workflow.rs) | YAML workflow definition + compilation into DAG tasks |
| [ork-core::Database (trait)](../../crates/ork-core/src/database.rs) | Storage contract for runs/tasks/workflows used by Scheduler |
| [ork-state::PostgresDatabase](../../crates/ork-state/src/postgres.rs) | Default Database implementation for production Postgres |
| [ork-state::FileDatabase](../../crates/ork-state/src/file_database.rs) | File-backed Database for local/dev use |
| [ork-executors::ExecutorManager](../../crates/ork-executors/src/manager.rs) | Chooses executor per task and caches backends |
| [ork-executors::ProcessExecutor](../../crates/ork-executors/src/process.rs) | Executes local scripts or Python tasks via [scripts/run_python_task.py](../../scripts/run_python_task.py), reports status |
| [ork-executors::CloudRunClient](../../crates/ork-executors/src/cloud_run.rs) | Creates and polls Cloud Run executions, reports status |
| [ork-runner::LocalScheduler (legacy)](../../crates/ork-runner/src/scheduler.rs) | Old scheduler used by ork-web; pending migration |
| [ork-web (legacy)](crates/ork-web.md) | Axum UI/API built on ork-runner |

## Data Flow

### Workflow Definition Flow (YAML -> DB)

```mermaid
sequenceDiagram
    participant OrkCLI as ork-cli (ork)
    participant Compiler as ork-core::workflow
    participant DB as ork-state::PostgresDatabase

    OrkCLI->>Compiler: load + validate YAML
    Compiler-->>OrkCLI: CompiledWorkflow
    OrkCLI->>DB: create_workflow(...)
    OrkCLI->>DB: create_workflow_tasks(compiled DAG)
```

### Run Creation Flow (Scheduler)

```mermaid
sequenceDiagram
    participant Scheduler as ork-core::Scheduler
    participant DB as ork-state::PostgresDatabase

    Scheduler->>DB: get_pending_runs()
    DB-->>Scheduler: [Run]
    Scheduler->>DB: get_workflows_by_ids(...)
    Scheduler->>DB: list_workflow_tasks(workflow_id)
    Scheduler->>DB: batch_create_dag_tasks(run_id, tasks)
    Scheduler->>DB: update_run_status(running)
```

### Task Execution Flow (Per-task executors)

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

    Scheduler->>ExecMgr: get_executor(task.executor_type, workflow)
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

- **Compile DAGs up front**: [`create-workflow-yaml`](../../crates/ork-cli/src/main.rs) compiles YAML into `workflow_tasks` rows.
- **Per-task executors**: [`Task.executor_type`](../../crates/ork-core/src/models.rs) drives backend selection per task.
- **Event-driven updates**: [`ork-core::Scheduler`](../../crates/ork-core/src/scheduler.rs) listens on an `mpsc::unbounded_channel` for `StatusUpdate`s.
- **Batch DB ops**: [`ork-state::PostgresDatabase`](../../crates/ork-state/src/postgres.rs) implements batch create/update to reduce round trips.
- **Bounded concurrency**: dispatch uses `buffer_unordered` controlled by [`OrchestratorConfig`](../../crates/ork-core/src/config.rs).

## Current Limitations

- No retry logic (tasks fail permanently)
- No timeout handling (tasks can run indefinitely)
- No heartbeat monitoring (hung tasks are not detected)
- No output passing between tasks (ordering-only DAGs)
- No scheduled runs (manual trigger only)
