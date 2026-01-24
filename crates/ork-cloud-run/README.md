# ork-cloud-run

Standalone Rust orchestrator for Cloud Run jobs with PostgreSQL state.

## Features

- **Dual executors**: Cloud Run jobs or local processes
- **Low latency**: Eliminates N+1 queries, bounded concurrency
- **Low memory**: Batch limits, connection pooling, streaming

## Quick Start

```bash
# Setup
just setup

# Run orchestrator
just run

# Trigger workflow (in another terminal)
just trigger example-process

# Check status
just status
```

## Commands

```bash
just --list              # Show all commands
just setup               # First-time setup
just run                 # Start orchestrator
just run-optimized       # Low-latency mode
just trigger <workflow>  # Trigger workflow
just status              # View runs
just test-load           # Load test (100 workflows)
```

## CLI

```bash
# Workflows
cargo run -- create-workflow --name <name> --job-name <job> --project <project> --executor <cloudrun|process>
cargo run -- list-workflows
cargo run -- trigger <workflow-name>

# Runs & Tasks
cargo run -- status [run-id]
cargo run -- tasks <run-id>

# Database
cargo run -- init
```

## Configuration

Set via environment:
- `DATABASE_URL` - Postgres connection (default: localhost:5432)
- `RUST_LOG` - Log level (info, debug)

## Performance

Key decisions:
1. **JOIN queries** - Fetch task+workflow in one query (not N+1)
2. **Bounded concurrency** - `buffer_unordered(N)` prevents memory explosion
3. **Batch limits** - Process max N tasks per poll
4. **Connection pooling** - Reuse DB connections
5. **Strategic indexes** - Composite indexes on hot paths

See [PERFORMANCE.md](PERFORMANCE.md) for details.

## Docker

```bash
docker compose up -d     # Start postgres + orchestrator
docker compose exec orchestrator /app/rust-orchestrator <command>
```

## Project Structure

```
src/
  main.rs              # CLI
  scheduler.rs         # Polling loop
  db.rs               # Database queries
  models.rs           # Data types
  cloud_run.rs        # Cloud Run executor
  process_executor.rs # Process executor
  config.rs           # Performance tuning
migrations/           # Database schema
```
