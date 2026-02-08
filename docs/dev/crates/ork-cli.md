# ork-cli

The main host process for Ork. It owns the CLI surface and starts the scheduler loop.

## Owns

- CLI commands for workflows, runs, tasks, and DB init
- `run-workflow` command that posts YAML to the HTTP API and triggers a run
- Scheduler host wiring ([`ork-core::Scheduler`](../../../crates/ork-core/src/scheduler.rs) + [ork-state](ork-state.md) + [ork-executors](ork-executors.md))
- YAML workflow compilation into `workflow_tasks`
- Postgres schema and migrations
- Perf test binary and configs

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [.dockerignore](../../../crates/ork-cli/.dockerignore) | Excludes local artifacts from the Docker build context. | 2026-02-06 | e773e90 |
| [Cargo.toml](../../../crates/ork-cli/Cargo.toml) | Crate manifest and feature wiring for the CLI binaries. | 2026-02-06 | 7341971 |
| [Dockerfile](../../../crates/ork-cli/Dockerfile) | Container image build for running the ork CLI/scheduler. | 2026-02-06 | 29c76d0 |
| [README.md](../../../crates/ork-cli/README.md) | Usage and development notes specific to the CLI. | 2026-02-06 | b111094 |
| [docker-compose.yml](../../../crates/ork-cli/docker-compose.yml) | Local Postgres service definition for development and tests. | 2026-02-06 | 73415d6 |
| [justfile](../../../crates/ork-cli/justfile) | Just tasks for running, testing, and performance workflows. | 2026-02-06 | 138d510 |
| [migrations/001_init.sql](../../../crates/ork-cli/migrations/001_init.sql) | Initial schema for workflows, runs, and tasks. | 2026-02-06 | d6ef2a5 |
| [migrations/002_add_executor_type.sql](../../../crates/ork-cli/migrations/002_add_executor_type.sql) | Adds executor type metadata to workflows. | 2026-02-06 | 6f194bd |
| [migrations/003_add_indexes.sql](../../../crates/ork-cli/migrations/003_add_indexes.sql) | Adds query indexes to speed up scheduling. | 2026-02-06 | 7a49c8a |
| [migrations/004_rename_executor_agnostic.sql](../../../crates/ork-cli/migrations/004_rename_executor_agnostic.sql) | Renames executor-specific columns to executor-agnostic names. | 2026-02-06 | 723370f |
| [migrations/005_add_dag_support.sql](../../../crates/ork-cli/migrations/005_add_dag_support.sql) | Adds DAG workflow columns and task dependencies. | 2026-02-06 | f24808f |
| [migrations/006_workflow_tasks.sql](../../../crates/ork-cli/migrations/006_workflow_tasks.sql) | Stores compiled DAGs and per-task executor type. | 2026-02-06 | 1662f24 |
| [migrations/007_add_task_logs.sql](../../../crates/ork-cli/migrations/007_add_task_logs.sql) | Adds task log storage to the tasks table. | 2026-02-06 | 823943e |
| [migrations/008_add_task_retries_timeouts.sql](../../../crates/ork-cli/migrations/008_add_task_retries_timeouts.sql) | Adds retry/timeout metadata to tasks. | 2026-02-06 | 6ceaaf9 |
| [migrations/009_add_deferred_jobs.sql](../../../crates/ork-cli/migrations/009_add_deferred_jobs.sql) | Adds deferred job tracking tables for deferrable tasks. | 2026-02-07 | 8a5fdcd |
| [perf-configs/heavy.yaml](../../../crates/ork-cli/perf-configs/heavy.yaml) | Load-test configuration for heavy stress runs. | 2026-02-06 | 0563280 |
| [perf-configs/latency.yaml](../../../crates/ork-cli/perf-configs/latency.yaml) | Load-test configuration for latency-focused runs. | 2026-02-06 | 7068a4c |
| [perf-configs/memory.yaml](../../../crates/ork-cli/perf-configs/memory.yaml) | Load-test configuration for memory-focused runs. | 2026-02-06 | 4d94588 |
| [perf-configs/quick.yaml](../../../crates/ork-cli/perf-configs/quick.yaml) | Load-test configuration for quick smoke runs. | 2026-02-06 | 6bb5184 |
| [perf-configs/standard.yaml](../../../crates/ork-cli/perf-configs/standard.yaml) | Load-test configuration for standard runs. | 2026-02-06 | 76d5f60 |
| [schema.sql](../../../crates/ork-cli/schema.sql) | Generated schema snapshot after applying all migrations. | 2026-02-06 | 9905605 |
| [scripts/dump-schema.sh](../../../crates/ork-cli/scripts/dump-schema.sh) | Generates `schema.sql` via pg_dump after migrations. | 2026-02-06 | b698ac4 |
| [scripts/test-e2e.sh](../../../crates/ork-cli/scripts/test-e2e.sh) | End-to-end CLI test that triggers and inspects a run. | 2026-02-06 | bd553d1 |
| [scripts/test-load.sh](../../../crates/ork-cli/scripts/test-load.sh) | Simple load test that triggers many workflows. | 2026-02-06 | 844ff75 |
| [src/bin/perf-test.rs](../../../crates/ork-cli/src/bin/perf-test.rs) | Performance test binary used by `just perf-*` workflows. | 2026-02-08 | c6274ca |
| [src/bin/perf_test/integration_tests.rs](../../../crates/ork-cli/src/bin/perf_test/integration_tests.rs) | Integration-style perf-test coverage using a mock `ork-cloud-run` binary and controlled config. | 2026-02-08 | 7f56161 |
| [src/bin/perf_test/perf_metrics.rs](../../../crates/ork-cli/src/bin/perf_test/perf_metrics.rs) | Helpers for parsing and summarizing scheduler metrics in perf-test output. | 2026-02-08 | f93003e |
| [src/bin/perf_test/perf_support.rs](../../../crates/ork-cli/src/bin/perf_test/perf_support.rs) | Perf-test support helpers for config loading, script setup, and throughput reporting. | 2026-02-08 | bbb0033 |
| [src/commands/create_workflow.rs](../../../crates/ork-cli/src/commands/create_workflow.rs) | Implements `ork create-workflow` for ad-hoc workflows. | 2026-02-06 | f58af8c |
| [src/commands/create_workflow_yaml.rs](../../../crates/ork-cli/src/commands/create_workflow_yaml.rs) | Implements `ork create-workflow-yaml` for YAML DAGs. | 2026-02-07 | 0bca210 |
| [src/commands/delete_workflow.rs](../../../crates/ork-cli/src/commands/delete_workflow.rs) | Implements `ork delete-workflow`. | 2026-02-06 | 97bad38 |
| [src/commands/execute.rs](../../../crates/ork-cli/src/commands/execute.rs) | Create, run, and wait for a workflow from YAML locally. | 2026-02-07 | aa45968 |
| [src/commands/init.rs](../../../crates/ork-cli/src/commands/init.rs) | Implements `ork init` migrations. | 2026-02-06 | cd4e52a |
| [src/commands/list_workflows.rs](../../../crates/ork-cli/src/commands/list_workflows.rs) | Implements `ork list-workflows`. | 2026-02-06 | 6c5261b |
| [src/commands/mod.rs](../../../crates/ork-cli/src/commands/mod.rs) | Command registry + dispatch helpers for CLI subcommands. | 2026-02-06 | c01c9e7 |
| [src/commands/run.rs](../../../crates/ork-cli/src/commands/run.rs) | Implements `ork run` scheduler startup. | 2026-02-06 | 6cda9fd |
| [src/commands/run_workflow.rs](../../../crates/ork-cli/src/commands/run_workflow.rs) | Implements `ork run-workflow` via the HTTP API. | 2026-02-07 | aac5a68 |
| [src/commands/status.rs](../../../crates/ork-cli/src/commands/status.rs) | Implements `ork status`. | 2026-02-06 | 58f326e |
| [src/commands/tasks.rs](../../../crates/ork-cli/src/commands/tasks.rs) | Implements `ork tasks`. | 2026-02-06 | f5bf9c3 |
| [src/commands/trigger.rs](../../../crates/ork-cli/src/commands/trigger.rs) | Implements `ork trigger`. | 2026-02-06 | fd656f4 |
| [src/main.rs](../../../crates/ork-cli/src/main.rs) | CLI entry point that parses commands, initializes storage, and starts the scheduler. | 2026-02-07 | 3c826db |
| [test-local.sh](../../../crates/ork-cli/test-local.sh) | Local walkthrough script for exercising the process executor. | 2026-02-06 | fd8204e |
| [test-scripts/example-task.sh](../../../crates/ork-cli/test-scripts/example-task.sh) | Example process-executor task script used in tests. | 2026-02-06 | 0f38387 |
| [test-scripts/perf-task.sh](../../../crates/ork-cli/test-scripts/perf-task.sh) | Lightweight task script used in perf runs. | 2026-02-06 | 487b94d |
| [tests/main_entrypoint_test.rs](../../../crates/ork-cli/tests/main_entrypoint_test.rs) | Binary-entrypoint smoke tests for `ork --help` and `run-workflow` pre-DB failure behavior. | 2026-02-08 | 0000000 |

## Notes

- Depends on [ork-core](ork-core.md), [ork-state](ork-state.md), and [ork-executors](ork-executors.md).
- CLI usage details live in [crates/ork-cli/README.md](../../../crates/ork-cli/README.md).
