# ork-executors

Execution backends and the executor manager. Implements [`ork-core::Executor`](../../../crates/ork-core/src/executor.rs).

## Owns

- `ExecutorManager`: per-task executor selection and caching
- `ProcessExecutor`: local process runner for scripts and Python tasks
- `CloudRunClient`: Cloud Run execution client (feature-gated)

## Behavior Summary

- Scheduler sets a `StatusUpdate` channel on each executor instance.
- Executors send updates when tasks start or finish.
- `ExecutorManager` selects the backend based on task `executor_type`.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-executors/Cargo.toml) | Crate manifest and executor feature flags. | 2026-01-31 | 5a4e746 |
| [src/cloud_run.rs](../../../crates/ork-executors/src/cloud_run.rs) | Cloud Run executor client and polling loop. | 2026-02-01 | f83c1d6 |
| [src/lib.rs](../../../crates/ork-executors/src/lib.rs) | Re-exports executors and gates features. | 2026-01-31 | beafa5a |
| [src/manager.rs](../../../crates/ork-executors/src/manager.rs) | `ExecutorManager` implementation and backend selection per task. | 2026-01-31 | eda9b7d |
| [src/process.rs](../../../crates/ork-executors/src/process.rs) | Local script/Python execution and task status reporting. | 2026-02-01 | e4d1237 |
