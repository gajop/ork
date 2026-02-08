# ork-core

The base crate: models, traits, and the scheduler loop. Everything else builds on this.

## Owns

- Domain models (`Workflow`, `Run`, `Task`, `ExecutorType`)
- Status enums and helpers for string conversion
- Core traits: `Database`, `Executor`, `ExecutorManager`
- Scheduler loop (`Scheduler`) and `OrchestratorConfig`
- `StatusUpdate` message type
- YAML workflow definitions and compilation utilities

## Extension Points

- Implement [`Database`](../../../crates/ork-core/src/database.rs) in [ork-state](ork-state.md) (storage backends).
- Implement [`Executor`](../../../crates/ork-core/src/executor.rs) in [ork-executors](ork-executors.md) (execution backends).

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-core/Cargo.toml) | Crate manifest and feature flags for core logic. | 2026-02-07 | 0464602 |
| [src/compiled.rs](../../../crates/ork-core/src/compiled.rs) | Compiles workflows into resolved tasks and a topo order. | 2026-02-07 | 7f52e04 |
| [src/compiled/tests.rs](../../../crates/ork-core/src/compiled/tests.rs) | Unit tests for workflow compilation helpers and Python signature introspection error paths. | 2026-02-08 | 0000000 |
| [src/config.rs](../../../crates/ork-core/src/config.rs) | Scheduler configuration defaults and tuning knobs. | 2026-02-06 | a4d0e63 |
| [src/database.rs](../../../crates/ork-core/src/database.rs) | Storage contract consumed by the scheduler. | 2026-02-07 | eeef5b2 |
| [src/error.rs](../../../crates/ork-core/src/error.rs) | Error types for workflow loading and execution. | 2026-02-06 | 1ed0edd |
| [src/executor.rs](../../../crates/ork-core/src/executor.rs) | Executor interface and the `StatusUpdate` message. | 2026-02-06 | 567d8ec |
| [src/executor_manager.rs](../../../crates/ork-core/src/executor_manager.rs) | Executor manager trait for per-task executors. | 2026-02-06 | d1d5a9a |
| [src/job_tracker.rs](../../../crates/ork-core/src/job_tracker.rs) | Deferrable job tracker implementations for external systems and custom HTTP polling. | 2026-02-07 | a296fea |
| [src/job_tracker/tests.rs](../../../crates/ork-core/src/job_tracker/tests.rs) | Unit tests for custom HTTP tracker validation behavior. | 2026-02-07 | e3549df |
| [src/lib.rs](../../../crates/ork-core/src/lib.rs) | Module wiring and public exports for ork-core. | 2026-02-07 | 6c75c9d |
| [src/models.rs](../../../crates/ork-core/src/models.rs) | Database-backed models for workflows, runs, and tasks. | 2026-02-07 | c3035d6 |
| [src/schedule_processor.rs](../../../crates/ork-core/src/schedule_processor.rs) | Cron schedule processing and run triggering. | 2026-02-06 | 4f3f5a2 |
| [src/scheduler.rs](../../../crates/ork-core/src/scheduler.rs) | Main scheduler loop and status update handling. | 2026-02-07 | b10017f |
| [src/scheduler/processing.rs](../../../crates/ork-core/src/scheduler/processing.rs) | Scheduler processing internals for runs, tasks, statuses, and deferred completions. | 2026-02-07 | 2590dc4 |
| [src/task_execution.rs](../../../crates/ork-core/src/task_execution.rs) | Task dispatch preparation and executor invocation. | 2026-02-07 | 01212ad |
| [src/task_execution/tests.rs](../../../crates/ork-core/src/task_execution/tests.rs) | Unit tests for task execution parameter shaping, retry behavior, and executor-manager errors. | 2026-02-08 | 0000000 |
| [src/triggerer.rs](../../../crates/ork-core/src/triggerer.rs) | Triggerer loop that polls deferred jobs and reports completion back to the scheduler. | 2026-02-07 | b44bbd3 |
| [src/types.rs](../../../crates/ork-core/src/types.rs) | In-memory run/task types and status enums for legacy flows. | 2026-02-06 | a83f970 |
| [src/worker_client.rs](../../../crates/ork-core/src/worker_client.rs) | Client for submitting tasks to external worker processes. | 2026-02-07 | 181eb0f |
| [src/workflow.rs](../../../crates/ork-core/src/workflow.rs) | Workflow definition loading and validation. | 2026-02-07 | 51d5121 |
| [tests/compiled_workflow_test.rs](../../../crates/ork-core/tests/compiled_workflow_test.rs) | Integration tests for workflow compilation success and missing-file error behavior. | 2026-02-08 | 0000000 |
| [tests/dag_execution_test.rs](../../../crates/ork-core/tests/dag_execution_test.rs) | DAG execution order/performance integration test. | 2026-02-07 | e57a0cb |
| [tests/deferrables_test.rs](../../../crates/ork-core/tests/deferrables_test.rs) | Integration tests for deferrable task lifecycle handling. | 2026-02-07 | e13ff26 |
| [tests/pause_resume_test.rs](../../../crates/ork-core/tests/pause_resume_test.rs) | Pause/resume scheduler integration tests. | 2026-02-07 | a0b6a39 |
| [tests/schedule_processor_test.rs](../../../crates/ork-core/tests/schedule_processor_test.rs) | Integration tests for scheduled workflow processing and trigger time updates. | 2026-02-07 | 4b0a3f8 |
| [tests/scheduler_channel_behavior_test.rs](../../../crates/ork-core/tests/scheduler_channel_behavior_test.rs) | Integration tests for scheduler channel ingestion paths (status updates and deferred completion events). | 2026-02-08 | 0000000 |
| [tests/scheduler_processing_behavior_test.rs](../../../crates/ork-core/tests/scheduler_processing_behavior_test.rs) | Integration tests for scheduler processing behavior: dispatch failures, retries, deferrals, and timeout enforcement. | 2026-02-07 | bfa650e |
| [tests/scheduler_startup_test.rs](../../../crates/ork-core/tests/scheduler_startup_test.rs) | Integration tests for scheduler startup behavior and triggerer initialization path. | 2026-02-08 | 0000000 |
| [tests/sqlite_deferred_jobs_test.rs](../../../crates/ork-core/tests/sqlite_deferred_jobs_test.rs) | SQLite integration tests for deferred job lifecycle and cancellation behavior. | 2026-02-07 | 7756af0 |
| [tests/triggerer_behavior_test.rs](../../../crates/ork-core/tests/triggerer_behavior_test.rs) | Integration tests for triggerer polling outcomes: running, completion, failure, timeout, and missing tracker behavior. | 2026-02-07 | 0000000 |
| [tests/type_extraction_test.rs](../../../crates/ork-core/tests/type_extraction_test.rs) | Tests for workflow/task type extraction and signature metadata behavior. | 2026-02-07 | 1c4ab3e |
