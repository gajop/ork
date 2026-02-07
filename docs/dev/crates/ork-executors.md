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
| [Cargo.toml](../../../crates/ork-executors/Cargo.toml) | Crate manifest and executor feature flags. | 2026-02-07 | 7cebb3f |
| [runtime/python/inspect_task.py](../../../crates/ork-executors/runtime/python/inspect_task.py) | Python helper used to introspect task signatures and metadata. | 2026-02-06 | b8e7802 |
| [runtime/python/run_task.py](../../../crates/ork-executors/runtime/python/run_task.py) | Python runtime entrypoint for executing typed task functions. | 2026-02-06 | 356372d |
| [src/cloud_run.rs](../../../crates/ork-executors/src/cloud_run.rs) | Cloud Run executor client and polling loop. | 2026-02-06 | 7dd0605 |
| [src/lib.rs](../../../crates/ork-executors/src/lib.rs) | Re-exports executors and gates features. | 2026-02-07 | fb508b0 |
| [src/library.rs](../../../crates/ork-executors/src/library.rs) | Dynamic library executor for loading and invoking task symbols. | 2026-02-07 | 85acf78 |
| [src/manager.rs](../../../crates/ork-executors/src/manager.rs) | `ExecutorManager` implementation and backend selection per task. | 2026-02-07 | 4189265 |
| [src/process.rs](../../../crates/ork-executors/src/process.rs) | Local script/Python execution and task status reporting. | 2026-02-07 | 41c68d8 |
| [src/python_runtime.rs](../../../crates/ork-executors/src/python_runtime.rs) | Python runtime orchestration and environment/bootstrap helpers. | 2026-02-06 | eb0b9e2 |
| [src/worker.rs](../../../crates/ork-executors/src/worker.rs) | Worker executor implementation for remote task execution mode. | 2026-02-07 | af13705 |
| [tests/test_library.rs](../../../crates/ork-executors/tests/test_library.rs) | Integration tests for the dynamic library executor path. | 2026-02-07 | 99f8f2c |
