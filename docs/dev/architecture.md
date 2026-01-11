Status: Pending Review

# Architecture

Ork is a serverless workflow orchestrator designed for near-zero idle cost.

## Components

```mermaid
architecture-beta
    group control[Control Plane]
    group workers[Workers]
    group state[State]

    service api(server)[API] in control
    service sched(server)[Scheduler] in control

    service w1(server)[Worker] in workers
    service w2(server)[Worker] in workers
    service w3(server)[Worker] in workers

    service db(database)[State Store] in state
    service storage(disk)[Object Store] in state

    api:B -- T:db
    sched:B -- T:db
    sched:B -- T:storage
    sched:R -- L:w1
    sched:R -- L:w2
    sched:R -- L:w3
    w1:B -- T:storage
    w2:B -- T:storage
    w3:B -- T:storage
```

## Component Responsibilities

| Component | Role |
|-----------|------|
| **Scheduler** | Polls for work, dispatches tasks, monitors completion |
| **Worker** | Executes task code, reports status |
| **API** | User interface, manual triggers, scheduler control |
| **State Store** | Workflow definitions, run metadata, schedules, leases |
| **Object Store** | Task specs, status files, outputs |

## Data Flow

```mermaid
sequenceDiagram
    participant S as Scheduler
    participant DB as State Store
    participant OS as Object Store
    participant W as Worker

    loop every 60s
        S->>DB: check due schedules
        S->>DB: create run + tasks
    end

    loop every 5s
        S->>DB: find pending tasks
        S->>DB: acquire task (SKIP LOCKED)
        S->>OS: write spec.json
        S->>W: create worker job
    end

    W->>OS: read spec.json
    W->>OS: write status.json (running)

    loop every 10s
        W->>OS: update heartbeat
    end

    W->>W: execute task code
    W->>OS: write output.json
    W->>OS: write status.json (success/failed)

    loop every 10s
        S->>OS: poll status.json
        S->>DB: update task state
        S->>DB: check run completion
    end
```

## Failure Handling

```mermaid
flowchart LR
    subgraph detection[Detection]
        HB[Heartbeat stale]
        TO[Timeout exceeded]
        EX[Non-zero exit]
    end

    subgraph recovery[Recovery]
        RETRY[Retry task]
        FAIL[Mark failed]
        ALERT[Alert]
    end

    HB --> RETRY
    TO --> FAIL
    EX --> RETRY
    RETRY -->|max retries exceeded| FAIL
    FAIL --> ALERT
```
