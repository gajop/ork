# ork-cli

The `ork` binary - a high-performance task orchestrator CLI.

## Quick Start

```bash
# Setup database
just setup

# Run orchestrator
just run

# Trigger workflow (in another terminal)
just trigger example-process

# Check status
just status
```

## CLI Commands

```bash
# Workflows
ork create-workflow --name <name> --job-name <job> --executor <cloudrun|process>
ork create-workflow-yaml --file <workflow.yaml> [--project local] [--region local]
ork list-workflows
ork trigger <workflow-name>

# Runs & Tasks
ork status [run-id]
ork tasks <run-id>

# Database
ork init
ork run  # Start scheduler
```

## Development

```bash
just --list              # Show all commands
just setup               # First-time setup
just run                 # Start orchestrator
just trigger <workflow>  # Trigger workflow
just status              # View runs
just test-load           # Load test (100 workflows)

# Performance testing
just perf-quick          # Quick smoke test
just perf-standard       # Standard load test
just perf-all            # Run all perf tests
```

See [../../docs/performance.md](../../docs/performance.md) for performance testing details.

## Configuration

Environment variables:
- `DATABASE_URL` - Postgres connection (default: localhost:5432)
- `RUST_LOG` - Log level (info, debug)

## Architecture

- **Database-backed**: PostgreSQL or SQLite for state
- **Event-driven**: Channel-based status updates from executors
- **Optimized**: JOIN queries, bounded concurrency, connection pooling
- **Multiple executors**: Process (local), Cloud Run (serverless)

See [../../docs/dev/architecture.md](../../docs/dev/architecture.md) for details.
