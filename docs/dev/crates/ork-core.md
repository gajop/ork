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
| [Cargo.toml](../../../crates/ork-core/Cargo.toml) | Crate manifest and feature flags for core logic. | 2026-01-31 | 45356e5 |
| [src/compiled.rs](../../../crates/ork-core/src/compiled.rs) | Compiles workflows into resolved tasks and a topo order. | 2026-01-31 | 5047420 |
| [src/config.rs](../../../crates/ork-core/src/config.rs) | Scheduler configuration defaults and tuning knobs. | 2026-01-31 | b984fc8 |
| [src/database.rs](../../../crates/ork-core/src/database.rs) | Storage contract consumed by the scheduler. | 2026-01-31 | cf6046e |
| [src/error.rs](../../../crates/ork-core/src/error.rs) | Error types for workflow loading and execution. | 2026-01-31 | ac2d1e8 |
| [src/executor.rs](../../../crates/ork-core/src/executor.rs) | Executor interface and the `StatusUpdate` message. | 2026-01-31 | 749e6b0 |
| [src/executor_manager.rs](../../../crates/ork-core/src/executor_manager.rs) | Executor manager trait for per-task executors. | 2026-01-31 | d1d5a9a |
| [src/lib.rs](../../../crates/ork-core/src/lib.rs) | Module wiring and public exports for ork-core. | 2026-01-30 | a6bf28b |
| [src/models.rs](../../../crates/ork-core/src/models.rs) | Database-backed models for workflows, runs, and tasks. | 2026-01-31 | a73663f |
| [src/scheduler.rs](../../../crates/ork-core/src/scheduler.rs) | Main scheduler loop and status update handling. | 2026-01-31 | b21c18d |
| [src/types.rs](../../../crates/ork-core/src/types.rs) | In-memory run/task types and status enums for legacy flows. | 2026-01-30 | 158957a |
| [src/workflow.rs](../../../crates/ork-core/src/workflow.rs) | Workflow definition loading and validation. | 2026-01-31 | 3d12cb7 |
