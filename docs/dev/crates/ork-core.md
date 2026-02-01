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
| [Cargo.toml](../../../crates/ork-core/Cargo.toml) | Crate manifest and feature flags for core logic. | 2026-02-01 | 60825e3 |
| [src/compiled.rs](../../../crates/ork-core/src/compiled.rs) | Compiles workflows into resolved tasks and a topo order. | 2026-01-31 | 5047420 |
| [src/config.rs](../../../crates/ork-core/src/config.rs) | Scheduler configuration defaults and tuning knobs. | 2026-02-01 | a4d0e63 |
| [src/database.rs](../../../crates/ork-core/src/database.rs) | Storage contract consumed by the scheduler. | 2026-02-01 | 2008572 |
| [src/error.rs](../../../crates/ork-core/src/error.rs) | Error types for workflow loading and execution. | 2026-01-31 | ac2d1e8 |
| [src/executor.rs](../../../crates/ork-core/src/executor.rs) | Executor interface and the `StatusUpdate` message. | 2026-02-01 | 567d8ec |
| [src/executor_manager.rs](../../../crates/ork-core/src/executor_manager.rs) | Executor manager trait for per-task executors. | 2026-01-31 | d1d5a9a |
| [src/lib.rs](../../../crates/ork-core/src/lib.rs) | Module wiring and public exports for ork-core. | 2026-02-01 | 214feb3 |
| [src/models.rs](../../../crates/ork-core/src/models.rs) | Database-backed models for workflows, runs, and tasks. | 2026-02-01 | 80e9777 |
| [src/schedule_processor.rs](../../../crates/ork-core/src/schedule_processor.rs) | Cron schedule processing and run triggering. | 2026-02-01 | 4f3f5a2 |
| [src/scheduler.rs](../../../crates/ork-core/src/scheduler.rs) | Main scheduler loop and status update handling. | 2026-02-01 | 32e2e88 |
| [src/task_execution.rs](../../../crates/ork-core/src/task_execution.rs) | Task dispatch preparation and executor invocation. | 2026-02-01 | d40967c |
| [src/types.rs](../../../crates/ork-core/src/types.rs) | In-memory run/task types and status enums for legacy flows. | 2026-02-01 | a83f970 |
| [src/workflow.rs](../../../crates/ork-core/src/workflow.rs) | Workflow definition loading and validation. | 2026-02-01 | d96c125 |
| [tests/dag_execution_test.rs](../../../crates/ork-core/tests/dag_execution_test.rs) | DAG execution order/performance integration test. | 2026-02-01 | b3eb263 |
| [tests/pause_resume_test.rs](../../../crates/ork-core/tests/pause_resume_test.rs) | Pause/resume scheduler integration tests. | 2026-02-01 | 5954017 |
