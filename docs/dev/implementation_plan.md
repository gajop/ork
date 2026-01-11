Status: Pending Review

# Implementation Plan

## Phase 1: Core Types & Storage

Establish data models and storage layer.

### Tasks

1. **Core types** (`ork-core`)
   - `Workflow`, `TaskDefinition`, `WorkflowDefinition`
   - `Run`, `TaskRun`, `RunStatus`, `TaskStatus`
   - `Schedule`
   - `Context`, `TaskSpec`, `TaskStatusFile`
   - Serialization with serde

2. **State store trait** (`ork-state`)
   - `StateStore` trait definition
   - CRUD for workflows, runs, task_runs, schedules
   - Lease operations

3. **Firestore implementation**
   - Collection structure
   - Queries and transactions
   - Conditional updates for leases

4. **Object store trait** (`ork-object`)
   - `ObjectStore` trait definition
   - Typed wrappers for spec, status, output

5. **GCS implementation**
   - Read/write JSON objects
   - Local filesystem implementation for dev

### Deliverables

- `cargo test` passes for all types
- Can create workflow, run, tasks in Firestore emulator
- Can read/write task files to GCS or local filesystem

## Phase 2: Scheduler

Build the core scheduling loop.

### Tasks

1. **Scheduler structure** (`ork-scheduler`)
   - Main loop with configurable intervals
   - Graceful shutdown

2. **Cron evaluation**
   - Parse cron expressions
   - Find due schedules
   - Create runs with task runs

3. **Dependency resolution**
   - Find tasks with satisfied dependencies
   - Handle parallel task creation

4. **Task dispatch**
   - Atomic task acquisition
   - Write spec to object store

5. **Executor trait** (`ork-executor`)
   - `Executor` trait: `dispatch()`, `status()`, `cancel()`
   - Subprocess executor for local dev

6. **Task monitoring**
   - Poll status files
   - Detect completion, timeout, heartbeat failure
   - Update state store
   - Handle retries

7. **Lease management**
   - Acquire/renew/release run leases
   - Handle lease expiry

### Deliverables

- Scheduler runs locally with Firestore emulator
- Creates runs from cron schedules
- Dispatches tasks to subprocess executor
- Monitors completion and handles failures

## Phase 3: Worker

Build the worker that executes tasks.

### Tasks

1. **Worker binary** (`ork-worker`)
   - Read task spec from object store
   - Initialize context

2. **Heartbeat**
   - Background thread/task
   - Update status file periodically

3. **Python executor**
   - Load and call Python task
   - Handle Pydantic serialization
   - Capture stdout/stderr

4. **Rust executor**
   - Load and call Rust task
   - Handle serde serialization

5. **Status reporting**
   - Write status on start
   - Write output on success
   - Write error on failure

### Deliverables

- Worker binary runs locally
- Executes Python and Rust tasks
- Reports status via object store
- Handles errors and timeouts

## Phase 4: Cloud Run Integration

Deploy to GCP with Cloud Run Jobs.

### Tasks

1. **Cloud Run executor**
   - Implement `Executor` for Cloud Run Jobs API
   - Create job executions
   - Poll execution status

2. **Docker images**
   - Scheduler Dockerfile
   - Worker base Dockerfile
   - Multi-stage builds

3. **Deployment config**
   - Terraform for Firestore, GCS, Cloud Run
   - IAM permissions

4. **End-to-end test**
   - Deploy scheduler to Cloud Run
   - Run workflow with Cloud Run Jobs workers
   - Verify completion

### Deliverables

- Scheduler runs in Cloud Run
- Tasks execute as Cloud Run Jobs
- Full workflow completes in GCP

## Phase 5: CLI & API

Build user-facing interfaces.

### Tasks

1. **HTTP API** (`ork-api`)
   - axum server
   - REST endpoints for workflows, runs, tasks
   - Authentication (API keys or IAM)

2. **CLI** (`ork-cli`)
   - `ork workflow deploy/list/status/delete`
   - `ork run start/status/list/logs/cancel/retry`
   - `ork schema export`
   - `ork dev` for local development

3. **Workflow validation**
   - Parse and validate YAML
   - Check executor references
   - Validate type schemas

### Deliverables

- API deployed alongside scheduler
- CLI can manage workflows and runs
- Workflow validation with clear errors

## Phase 6: Python & Rust SDKs

Build the task authoring SDKs.

### Tasks

1. **Python SDK** (`ork-python`)
   - `@task` decorator
   - `Context` class
   - Pydantic integration
   - `@executor` decorator for custom executors

2. **Rust SDK** (`ork-sdk`)
   - `#[task]` macro
   - `Context` struct
   - serde integration
   - `#[executor]` macro

3. **Documentation**
   - SDK reference docs
   - Examples

### Deliverables

- Python SDK published to PyPI
- Rust SDK published to crates.io
- Getting started guides

## Phase 7: Additional Features

Extend functionality.

### Tasks

1. **dbt integration**
   - Parse dbt project
   - Extract model dependencies
   - Generate task definitions

2. **Parallelization**
   - Static parallel count
   - Dynamic by count reference
   - Dynamic by foreach items
   - Concurrency limits

3. **Additional state stores**
   - DynamoDB implementation
   - CosmosDB implementation
   - Postgres implementation

4. **Additional executors**
   - Fargate executor
   - Container Apps executor
   - Docker executor
   - Kubernetes Jobs executor

5. **Metrics & observability**
   - OpenTelemetry integration
   - Structured logging
   - Custom metrics from tasks

### Deliverables

- dbt projects can be imported
- Parallel tasks work correctly
- Multiple cloud providers supported

## Phase 8: UI

Build web interface.

### Tasks

1. **Frontend**
   - Workflow list and details
   - Run list and details
   - Task status and logs
   - DAG visualization

2. **Real-time updates**
   - Polling or WebSocket
   - Live task status

3. **Manual operations**
   - Trigger runs
   - Cancel runs
   - Retry tasks

### Deliverables

- Web UI deployed
- Can view and operate workflows

## Crate Dependency Graph

```
ork-core
    ↑
ork-state ←── ork-object
    ↑              ↑
ork-executor ──────┘
    ↑
ork-scheduler
    ↑
ork-worker
    
ork-api ←── ork-state, ork-scheduler
ork-cli ←── ork-api
```

## Testing Strategy

| Phase | Test Type |
|-------|-----------|
| 1 | Unit tests for types, integration tests with Firestore emulator |
| 2 | Integration tests with subprocess executor |
| 3 | Unit tests for worker, integration tests |
| 4 | End-to-end tests in GCP dev project |
| 5 | API integration tests, CLI tests |
| 6 | SDK unit tests, example projects |
| 7 | Feature-specific integration tests |
| 8 | Manual testing, possibly E2E with Playwright |
