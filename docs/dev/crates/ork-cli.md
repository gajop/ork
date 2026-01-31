# ork-cli

The main host process for Ork. It owns the CLI surface and starts the scheduler loop.

## Owns

- CLI commands for workflows, runs, tasks, and DB init
- Scheduler host wiring ([`ork-core::Scheduler`](../../../crates/ork-core/src/scheduler.rs) + [ork-state](ork-state.md) + [ork-executors](ork-executors.md))
- YAML workflow compilation into `workflow_tasks`
- Postgres schema and migrations
- Perf test binary and configs

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [.dockerignore](../../../crates/ork-cli/.dockerignore) | Excludes local artifacts from the Docker build context. | 2026-01-31 | e773e90 |
| [Cargo.toml](../../../crates/ork-cli/Cargo.toml) | Crate manifest and feature wiring for the CLI binaries. | 2026-01-31 | 03b8768 |
| [Dockerfile](../../../crates/ork-cli/Dockerfile) | Container image build for running the ork CLI/scheduler. | 2026-01-31 | 29c76d0 |
| [README.md](../../../crates/ork-cli/README.md) | Usage and development notes specific to the CLI. | 2026-01-31 | 32c8348 |
| [docker-compose.yml](../../../crates/ork-cli/docker-compose.yml) | Local Postgres service definition for development and tests. | 2026-01-31 | 73415d6 |
| [justfile](../../../crates/ork-cli/justfile) | Just tasks for running, testing, and performance workflows. | 2026-01-31 | 138d510 |
| [migrations/001_init.sql](../../../crates/ork-cli/migrations/001_init.sql) | Initial schema for workflows, runs, and tasks. | 2026-01-30 | d6ef2a5 |
| [migrations/002_add_executor_type.sql](../../../crates/ork-cli/migrations/002_add_executor_type.sql) | Adds executor type metadata to workflows. | 2026-01-30 | 6f194bd |
| [migrations/003_add_indexes.sql](../../../crates/ork-cli/migrations/003_add_indexes.sql) | Adds query indexes to speed up scheduling. | 2026-01-30 | 7a49c8a |
| [migrations/004_rename_executor_agnostic.sql](../../../crates/ork-cli/migrations/004_rename_executor_agnostic.sql) | Renames executor-specific columns to executor-agnostic names. | 2026-01-30 | 723370f |
| [migrations/005_add_dag_support.sql](../../../crates/ork-cli/migrations/005_add_dag_support.sql) | Adds DAG workflow columns and task dependencies. | 2026-01-31 | f24808f |
| [migrations/006_workflow_tasks.sql](../../../crates/ork-cli/migrations/006_workflow_tasks.sql) | Stores compiled DAGs and per-task executor type. | 2026-01-31 | 1662f24 |
| [perf-configs/heavy.yaml](../../../crates/ork-cli/perf-configs/heavy.yaml) | Load-test configuration for heavy stress runs. | 2026-01-30 | 0563280 |
| [perf-configs/latency.yaml](../../../crates/ork-cli/perf-configs/latency.yaml) | Load-test configuration for latency-focused runs. | 2026-01-30 | 7068a4c |
| [perf-configs/memory.yaml](../../../crates/ork-cli/perf-configs/memory.yaml) | Load-test configuration for memory-focused runs. | 2026-01-30 | 4d94588 |
| [perf-configs/quick.yaml](../../../crates/ork-cli/perf-configs/quick.yaml) | Load-test configuration for quick smoke runs. | 2026-01-30 | 6bb5184 |
| [perf-configs/standard.yaml](../../../crates/ork-cli/perf-configs/standard.yaml) | Load-test configuration for standard runs. | 2026-01-30 | 76d5f60 |
| [schema.sql](../../../crates/ork-cli/schema.sql) | Generated schema snapshot after applying all migrations. | 2026-01-31 | 9905605 |
| [scripts/dump-schema.sh](../../../crates/ork-cli/scripts/dump-schema.sh) | Generates `schema.sql` via pg_dump after migrations. | 2026-01-31 | b698ac4 |
| [scripts/test-e2e.sh](../../../crates/ork-cli/scripts/test-e2e.sh) | End-to-end CLI test that triggers and inspects a run. | 2026-01-30 | bd553d1 |
| [scripts/test-load.sh](../../../crates/ork-cli/scripts/test-load.sh) | Simple load test that triggers many workflows. | 2026-01-30 | 844ff75 |
| [src/bin/perf-test.rs](../../../crates/ork-cli/src/bin/perf-test.rs) | Performance test binary used by `just perf-*` workflows. | 2026-01-31 | 2adddd0 |
| [src/main.rs](../../../crates/ork-cli/src/main.rs) | CLI entry point that parses commands, initializes storage, and starts the scheduler. | 2026-01-31 | 2443f12 |
| [test-local.sh](../../../crates/ork-cli/test-local.sh) | Local walkthrough script for exercising the process executor. | 2026-01-30 | fd8204e |
| [test-scripts/example-task.sh](../../../crates/ork-cli/test-scripts/example-task.sh) | Example process-executor task script used in tests. | 2026-01-30 | 0f38387 |
| [test-scripts/perf-task.sh](../../../crates/ork-cli/test-scripts/perf-task.sh) | Lightweight task script used in perf runs. | 2026-01-30 | 487b94d |

## Notes

- Depends on [ork-core](ork-core.md), [ork-state](ork-state.md), and [ork-executors](ork-executors.md).
- CLI usage details live in [crates/ork-cli/README.md](../../../crates/ork-cli/README.md).
