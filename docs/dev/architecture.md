# Architecture

High-performance workflow orchestrator with event-driven scheduler and database-backed state.

## Components

```mermaid
graph TB
    subgraph orchestrator[Orchestrator Process]
        CLI[CLI / API]
        Scheduler[Event-Driven Scheduler]
        ExecMgr[Executor Manager]
    end

    subgraph executors[Executors]
        ProcExec[Process Executor]
        CloudExec[Cloud Run Executor]
    end

    subgraph workers[Workers]
        W1[Process 1]
        W2[Process 2]
        W3[Cloud Run Job]
    end

    subgraph storage[Storage Layer]
        DB[(Database)]
    end

    CLI --> DB
    CLI --> Scheduler
    Scheduler <--> DB
    Scheduler --> ExecMgr
    ExecMgr --> ProcExec
    ExecMgr --> CloudExec
    ProcExec -->|spawn & monitor| W1
    ProcExec -->|spawn & monitor| W2
    CloudExec -->|create & poll| W3
    ProcExec -.channel.-> Scheduler
    CloudExec -.channel.-> Scheduler
```

## Component Responsibilities

| Component | Role |
|-----------|------|
| **CLI** | User interface for workflow/run management |
| **Scheduler** | Event-driven task dispatcher, processes status updates via channels |
| **Executor Manager** | Manages executor instances per workflow |
| **Process Executor** | Spawns local processes, monitors via async tasks |
| **Cloud Run Executor** | Creates Cloud Run jobs, polls status |
| **Database** | Stores workflows, runs, tasks (Postgres or SQLite) |

## Data Flow

### Run Creation Flow

```mermaid
sequenceDiagram
    participant CLI
    participant DB as Database
    participant Scheduler
    participant ExecMgr as Executor Manager

    CLI->>DB: create_workflow(name, job, executor_type)
    CLI->>DB: create_run(workflow_id, triggered_by)

    Scheduler->>DB: get_pending_runs()
    DB-->>Scheduler: [Run]
    Scheduler->>DB: batch_create_tasks(run_id, task_count)
    Scheduler->>DB: update_run_status(running)
    Scheduler->>ExecMgr: register_workflow(workflow)
```

### Task Execution Flow (Event-Driven)

```mermaid
sequenceDiagram
    participant Scheduler
    participant DB as Database
    participant Executor
    participant Worker
    participant Channel

    loop poll interval (5s)
        Scheduler->>DB: get_pending_tasks_with_workflow(batch_size)
        DB-->>Scheduler: [TaskWithWorkflow]
    end

    par Concurrent Dispatch
        Scheduler->>Executor: execute(task_id, job_name, params)
        Executor->>Worker: spawn process / create Cloud Run job
        Executor->>DB: update_task_status(dispatched)
        Executor-->>Scheduler: execution_name
    end

    Worker->>Worker: run task
    Worker-->>Executor: exit code / completion

    Executor->>Channel: send StatusUpdate{task_id, status}
    Channel-->>Scheduler: receive StatusUpdate

    Scheduler->>DB: batch_update_task_status(updates)
    Scheduler->>DB: get_run_task_stats(run_id)

    alt all tasks complete
        Scheduler->>DB: update_run_status(success/failed)
    end
```

## Key Design Decisions

### Event-Driven Architecture
- **Channel-based updates**: Executors push status updates via `mpsc::unbounded_channel`
- **No polling**: Scheduler receives updates immediately when tasks complete
- **Async/await**: All I/O is non-blocking using Tokio runtime

### Database Optimizations
- **JOIN queries**: Single query fetches task + workflow data (not N+1)
- **Batch operations**: `batch_create_tasks`, `batch_update_task_status`
- **Bounded concurrency**: `buffer_unordered(N)` limits parallel dispatches
- **Connection pooling**: sqlx pool reuses database connections

### Performance Characteristics
- **Throughput**: 155+ tasks/sec
- **Latency**: <700ms p99 (task create → complete)
- **Memory**: ~10MB RSS for 500 concurrent tasks
- **Idle cost**: Near-zero when no work pending

## Current Limitations

- No retry logic (tasks fail permanently)
- No timeout handling (tasks can run indefinitely)
- No heartbeat monitoring (can't detect hung tasks)
- No DAG support (tasks run independently)
- No scheduled runs (manual trigger only)
