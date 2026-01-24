# Rust Cloud Run Orchestrator

A simple Rust-based orchestrator that generates tasks and executes them as Google Cloud Run jobs, using PostgreSQL as the database backend.

## Features

- **Simple Task Generation**: Create workflows that generate multiple tasks
- **Cloud Run Integration**: Execute tasks as Cloud Run jobs with full API integration
- **PostgreSQL Backend**: Reliable state management with Postgres
- **CLI Interface**: Easy-to-use command-line interface for managing workflows and runs
- **Docker Compose**: Local testing environment with Postgres
- **Mock Mode**: Test locally without Cloud Run credentials

## Architecture

```
┌──────────────┐
│   CLI        │ ← Create workflows, trigger runs, check status
└──────┬───────┘
       │
┌──────▼───────┐
│  Scheduler   │ ← Polls for pending runs/tasks
│              │   Dispatches tasks to Cloud Run
└──────┬───────┘
       │
┌──────▼───────┐     ┌────────────────┐
│  PostgreSQL  │◄────┤  Cloud Run API │
│              │     │  (Jobs)        │
└──────────────┘     └────────────────┘
```

## Prerequisites

- Rust 1.70+ (for local development)
- Docker and Docker Compose (for running with containers)
- Google Cloud credentials (for actual Cloud Run execution)

## Quick Start with Docker Compose

### 1. Start the services

```bash
# Start PostgreSQL and the orchestrator
docker-compose up -d

# Initialize the database
docker-compose exec orchestrator /app/rust-orchestrator init
```

### 2. Create a workflow

```bash
docker-compose exec orchestrator /app/rust-orchestrator create-workflow \
  --name "example-workflow" \
  --description "Example workflow" \
  --job-name "my-cloud-run-job" \
  --project "my-gcp-project" \
  --region "us-central1" \
  --task-count 5
```

### 3. List workflows

```bash
docker-compose exec orchestrator /app/rust-orchestrator list-workflows
```

### 4. Trigger a workflow

```bash
docker-compose exec orchestrator /app/rust-orchestrator trigger example-workflow
```

### 5. Check status

```bash
# View all runs
docker-compose exec orchestrator /app/rust-orchestrator status

# View specific run
docker-compose exec orchestrator /app/rust-orchestrator status <run-id>

# View tasks for a run
docker-compose exec orchestrator /app/rust-orchestrator tasks <run-id>
```

## Local Development

### 1. Install dependencies

```bash
cargo build
```

### 2. Start PostgreSQL

```bash
docker-compose up -d postgres
```

### 3. Initialize the database

```bash
cargo run -- init
```

### 4. Create a workflow

```bash
cargo run -- create-workflow \
  --name "test-workflow" \
  --job-name "my-job" \
  --project "my-project" \
  --task-count 3
```

### 5. Run the orchestrator

```bash
cargo run -- run
```

In another terminal:

```bash
# Trigger a workflow
cargo run -- trigger test-workflow

# Check status
cargo run -- status

# View tasks
cargo run -- tasks <run-id>
```

## CLI Commands

### Initialize database

```bash
rust-orchestrator init
```

### Run the scheduler

```bash
rust-orchestrator run
```

### Create a workflow

```bash
rust-orchestrator create-workflow \
  --name <name> \
  --description <description> \
  --job-name <cloud-run-job-name> \
  --project <gcp-project-id> \
  --region <region> \
  --task-count <number>
```

### List workflows

```bash
rust-orchestrator list-workflows
```

### Trigger a workflow run

```bash
rust-orchestrator trigger <workflow-name>
```

### Check run status

```bash
# All runs
rust-orchestrator status

# Specific run
rust-orchestrator status <run-id>

# Runs for a workflow
rust-orchestrator status --workflow <workflow-name>
```

### View tasks for a run

```bash
rust-orchestrator tasks <run-id>
```

## Configuration

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string (default: `postgres://postgres:postgres@localhost:5432/orchestrator`)
- `GOOGLE_APPLICATION_CREDENTIALS`: Path to GCP service account key (for Cloud Run API access)
- `RUST_LOG`: Log level (default: `info`)

## Google Cloud Setup

### 1. Create a Cloud Run Job

```bash
gcloud run jobs create my-job \
  --image gcr.io/my-project/my-image \
  --region us-central1 \
  --project my-project
```

### 2. Set up authentication

```bash
# Option 1: Application Default Credentials
gcloud auth application-default login

# Option 2: Service Account Key
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account-key.json
```

### 3. Grant permissions

Ensure your service account has the `run.jobs.run` permission:

```bash
gcloud projects add-iam-policy-binding my-project \
  --member="serviceAccount:my-service-account@my-project.iam.gserviceaccount.com" \
  --role="roles/run.developer"
```

## Mock Mode (Local Testing)

When `GOOGLE_APPLICATION_CREDENTIALS` is not set, the orchestrator runs in mock mode:

- Cloud Run API calls are simulated
- Tasks are marked as dispatched with mock execution names
- Status checks automatically return "success"
- Great for testing the orchestrator logic without GCP credentials

## Database Schema

### Workflows

- `id`: UUID primary key
- `name`: Unique workflow name
- `cloud_run_job_name`: Name of the Cloud Run job to execute
- `cloud_run_project`: GCP project ID
- `cloud_run_region`: GCP region
- `task_params`: JSON parameters (e.g., task count)

### Runs

- `id`: UUID primary key
- `workflow_id`: Foreign key to workflows
- `status`: pending, running, success, failed, cancelled
- `started_at`, `finished_at`: Timestamps

### Tasks

- `id`: UUID primary key
- `run_id`: Foreign key to runs
- `task_index`: Sequential task number
- `status`: pending, dispatched, running, success, failed
- `cloud_run_execution_name`: Cloud Run execution identifier
- `params`: JSON task parameters
- `dispatched_at`, `started_at`, `finished_at`: Timestamps

## How It Works

1. **Workflow Creation**: Define a workflow with Cloud Run job details and task generation parameters
2. **Run Triggering**: Create a run for a workflow (status: pending)
3. **Scheduler Loop** (every 5 seconds):
   - Process pending runs → Generate tasks based on workflow config
   - Process pending tasks → Dispatch to Cloud Run API
   - Check running tasks → Poll Cloud Run for status updates
   - Complete runs when all tasks finish
4. **Task Execution**: Each task is executed as a separate Cloud Run job execution

## Development

### Project Structure

```
.
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── migrations/
│   └── 001_init.sql
└── src/
    ├── main.rs          # CLI and main entry point
    ├── db.rs            # Database operations
    ├── models.rs        # Data models
    ├── scheduler.rs     # Scheduler loop logic
    └── cloud_run.rs     # Cloud Run API integration
```

### Testing

```bash
# Build
cargo build

# Run tests (when available)
cargo test

# Run with logging
RUST_LOG=debug cargo run -- run
```

## Stopping

```bash
# Stop all services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

## License

MIT
