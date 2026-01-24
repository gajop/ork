# Rust Cloud Run Orchestrator

A simple Rust-based orchestrator that generates tasks and executes them as **Google Cloud Run jobs** or **local processes**, using PostgreSQL as the database backend.

## Features

- **Dual Execution Modes**:
  - **Cloud Run**: Execute tasks as Google Cloud Run jobs via API
  - **Process**: Execute tasks as local shell scripts/processes
- **Simple Task Generation**: Create workflows that generate multiple tasks
- **PostgreSQL Backend**: Reliable state management with async Postgres operations
- **CLI Interface**: Easy-to-use command-line interface for managing workflows and runs
- **Docker Compose**: Local testing environment with Postgres
- **Justfile**: Convenient task runner with all common operations
- **Mock Mode**: Test Cloud Run mode locally without GCP credentials

## Architecture

```
┌──────────────┐
│   CLI        │ ← Create workflows, trigger runs, check status
└──────┬───────┘
       │
┌──────▼───────┐
│  Scheduler   │ ← Polls for pending runs/tasks (every 5s)
│              │   Dispatches to executor based on type
└──────┬───────┘
       │
       ├──────────────┬──────────────┐
       │              │              │
┌──────▼───────┐  ┌──▼────────┐  ┌──▼──────────┐
│  PostgreSQL  │  │ Cloud Run │  │   Process   │
│              │  │    API    │  │  Executor   │
└──────────────┘  └───────────┘  └─────────────┘
```

## Prerequisites

- **Rust 1.70+** (for local development)
- **Docker and Docker Compose** (for running with PostgreSQL)
- **Just** (task runner) - `cargo install just` (recommended)
- **Google Cloud credentials** (optional, only for Cloud Run executor)

## Quick Start with Justfile

The easiest way to use this project is with [just](https://github.com/casey/just), a command runner.

### Install Just

```bash
cargo install just
```

### List all available commands

```bash
just --list
```

### Common Workflows

#### 1. Local Development Setup

```bash
# Complete first-time setup
just setup

# This will:
# - Start PostgreSQL
# - Run migrations
# - Create test scripts
# - Create example workflow
```

#### 2. Run with Process Executor (Local Testing)

```bash
# Start the orchestrator
just run

# In another terminal, trigger the example workflow
just trigger example-process

# Check status
just status

# View tasks for a run
just tasks <run-id>
```

#### 3. Create Custom Workflows

```bash
# Process executor
just create-example-workflow

# Cloud Run executor
just create-cloudrun-workflow my-workflow my-cloud-run-job my-gcp-project us-central1 5
```

#### 4. Database Operations

```bash
# View workflows
just list-workflows

# Query database directly
just db-workflows
just db-runs
just db-tasks <run-id>
```

## Manual Usage (without Just)

### Local Development

#### 1. Start PostgreSQL

```bash
docker-compose up -d postgres
```

#### 2. Initialize the database

```bash
cargo run -- init
```

#### 3. Create a workflow

**Process Executor** (runs local scripts):
```bash
cargo run -- create-workflow \
  --name "example-process" \
  --description "Example with process executor" \
  --job-name "example-task.sh" \
  --project "local" \
  --region "local" \
  --task-count 5 \
  --executor process
```

**Cloud Run Executor** (runs Cloud Run jobs):
```bash
cargo run -- create-workflow \
  --name "my-workflow" \
  --description "Production workflow" \
  --job-name "my-cloud-run-job" \
  --project "my-gcp-project" \
  --region "us-central1" \
  --task-count 3 \
  --executor cloudrun
```

#### 4. Run the orchestrator

```bash
cargo run -- run
```

#### 5. Trigger a workflow (in another terminal)

```bash
cargo run -- trigger example-process

# Note the Run ID from output
```

#### 6. Check status

```bash
# All runs
cargo run -- status

# Specific run
cargo run -- status <run-id>

# Runs for a specific workflow
cargo run -- status --workflow example-process

# View tasks
cargo run -- tasks <run-id>
```

## Docker Compose Usage

### Start Everything

```bash
just docker-up

# Or manually:
docker-compose up -d
docker-compose exec orchestrator /app/rust-orchestrator init
```

### Execute Commands

```bash
just docker-exec list-workflows
just docker-exec status

# Or manually:
docker-compose exec orchestrator /app/rust-orchestrator <command>
```

## Executors

### Process Executor

Executes tasks as local shell scripts. Perfect for testing and local development.

**Features:**
- Runs scripts from `test-scripts/` directory
- Passes task parameters as environment variables
- Background execution with logging
- Simulated immediate completion (for demo purposes)

**Environment Variables Passed:**
- `task_index`: Task index number
- `run_id`: Run UUID
- `workflow_name`: Workflow name
- `EXECUTION_ID`: Unique execution UUID

**Example Script:**
```bash
#!/bin/bash
echo "Task index: ${task_index}"
echo "Run ID: ${run_id}"
echo "Workflow: ${workflow_name}"

# Do work
sleep 2

# Output JSON result
echo '{"status": "success", "processed": 42}'
```

### Cloud Run Executor

Executes tasks as Google Cloud Run jobs via the Cloud Run API.

**Features:**
- Full Cloud Run API integration
- Automatic authentication via `gcloud` CLI
- Environment variable passing to jobs
- Status polling for completion
- Mock mode when credentials unavailable

**Setup:**
```bash
# Authenticate with gcloud
gcloud auth application-default login

# Create a Cloud Run job
gcloud run jobs create my-job \
  --image gcr.io/my-project/my-image \
  --region us-central1
```

## CLI Commands Reference

| Command | Description |
|---------|-------------|
| `init` | Initialize database with migrations |
| `run` | Start the orchestrator scheduler |
| `create-workflow` | Create a new workflow |
| `list-workflows` | List all workflows |
| `trigger <name>` | Trigger a workflow run |
| `status [run-id]` | Show run status |
| `status --workflow <name>` | Show runs for a workflow |
| `tasks <run-id>` | Show tasks for a run |

## Configuration

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string
  Default: `postgres://postgres:postgres@localhost:5432/orchestrator`
- `RUST_LOG`: Log level (debug, info, warn, error)
  Default: `info`
- `GOOGLE_APPLICATION_CREDENTIALS`: Path to GCP service account key (for Cloud Run)

### Workflow Parameters

When creating a workflow:

- `--name`: Unique workflow identifier
- `--description`: Optional description
- `--job-name`: For Cloud Run: job name | For Process: script filename
- `--project`: For Cloud Run: GCP project ID | For Process: can be "local"
- `--region`: For Cloud Run: GCP region | For Process: can be "local"
- `--task-count`: Number of tasks to generate per run
- `--executor`: `cloudrun` or `process`

## Database Schema

### Workflows Table
- `id`: UUID
- `name`: Unique workflow name
- `executor_type`: `cloudrun` or `process`
- `cloud_run_job_name`: Job name or script path
- `cloud_run_project`: GCP project or "local"
- `cloud_run_region`: GCP region or "local"
- `task_params`: JSON (e.g., `{"task_count": 5}`)

### Runs Table
- `id`: UUID
- `workflow_id`: Foreign key to workflows
- `status`: pending → running → success/failed
- `started_at`, `finished_at`: Timestamps

### Tasks Table
- `id`: UUID
- `run_id`: Foreign key to runs
- `task_index`: Sequential task number (0, 1, 2, ...)
- `status`: pending → dispatched → running → success/failed
- `cloud_run_execution_name`: Execution identifier
- `params`: JSON task parameters
- `dispatched_at`, `started_at`, `finished_at`: Timestamps

## How It Works

1. **Workflow Creation**: Define workflows with executor type and task generation parameters
2. **Run Triggering**: Create a run for a workflow (status: pending)
3. **Scheduler Loop** (every 5 seconds):
   - **Process pending runs** → Generate N tasks based on workflow config
   - **Process pending tasks** → Dispatch to appropriate executor (Cloud Run API or Process)
   - **Check running tasks** → Poll for status updates
   - **Complete runs** → Mark run as success/failed when all tasks finish

4. **Task Execution**:
   - **Cloud Run**: API call to execute job, poll for completion
   - **Process**: Spawn shell script with env vars, immediate success (simulated)

## Project Structure

```
.
├── Cargo.toml              # Rust dependencies
├── Dockerfile              # Multi-stage Docker build
├── docker-compose.yml      # PostgreSQL + Orchestrator
├── justfile                # Task runner recipes
├── README.md               # This file
├── test-local.sh           # Local test demonstration
├── migrations/             # Database migrations
│   ├── 001_init.sql        # Initial schema
│   └── 002_add_executor_type.sql  # Add executor column
├── test-scripts/           # Example process executor scripts
│   └── example-task.sh     # Demo task script
└── src/
    ├── main.rs             # CLI and main entry point
    ├── db.rs               # Database operations (sqlx)
    ├── models.rs           # Data models (Workflow, Run, Task)
    ├── scheduler.rs        # Scheduler loop logic
    ├── cloud_run.rs        # Cloud Run API integration
    └── process_executor.rs # Local process execution
```

## Development

### Build

```bash
just build           # Debug build
just build-release   # Release build
```

### Code Quality

```bash
just check    # Check compilation
just fmt      # Format code
just lint     # Run clippy
just test     # Run tests
```

### Clean Up

```bash
just clean           # Remove build artifacts
just clean-test      # Remove test scripts
just reset-db        # Delete database (WARNING: loses data)
```

## Examples

### Process Executor Example

```bash
# Setup
just setup

# Start orchestrator
just run &

# Wait a moment, then trigger
sleep 2
just trigger example-process

# Check progress
just status
just tasks <run-id>
```

### Cloud Run Example

```bash
# Create Cloud Run workflow
cargo run -- create-workflow \
  --name prod-etl \
  --job-name data-processor \
  --project my-project \
  --region us-central1 \
  --task-count 10 \
  --executor cloudrun

# Trigger it
cargo run -- trigger prod-etl
```

## Troubleshooting

### Database Connection Issues

```bash
# Check if PostgreSQL is running
docker-compose ps

# Restart PostgreSQL
docker-compose restart postgres

# Check logs
docker-compose logs postgres
```

### Process Executor Not Working

```bash
# Ensure scripts are executable
chmod +x test-scripts/*.sh

# Check script exists
ls -la test-scripts/

# Check working directory setting in scheduler.rs
# Default: "test-scripts"
```

### Cloud Run Authentication

```bash
# Check gcloud auth
gcloud auth application-default print-access-token

# Re-authenticate if needed
gcloud auth application-default login
```

## License

MIT

## Contributing

This is a simple example orchestrator. Contributions welcome for:
- Better process tracking for Process executor
- Retry logic and error handling
- Task dependencies and DAG execution
- Web UI for monitoring
- More executor types (AWS Lambda, etc.)
