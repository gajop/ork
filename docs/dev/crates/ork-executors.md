# ork-executors

Execution backends and the executor manager. Implements [`ork-core::Executor`](../../../crates/ork-core/src/executor.rs).

## Owns

- `ExecutorManager`: per-workflow executor registry
- `ProcessExecutor`: local process runner for scripts
- `CloudRunClient`: Cloud Run execution client (feature-gated)

## Behavior Summary

- Scheduler sets a `StatusUpdate` channel on each executor.
- Executors send updates when tasks start or finish.
- `ExecutorManager` selects the backend based on workflow `executor_type`.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-executors/Cargo.toml) | Crate manifest and executor feature flags. | 2026-01-30 | 1e7d702 |
| [src/cloud_run.rs](../../../crates/ork-executors/src/cloud_run.rs) | Cloud Run executor client and polling loop. | 2026-01-30 | 69dffe0 |
| [src/lib.rs](../../../crates/ork-executors/src/lib.rs) | Re-exports executors and gates features. | 2026-01-30 | beafa5a |
| [src/manager.rs](../../../crates/ork-executors/src/manager.rs) | `ExecutorManager` implementation and backend selection per workflow. | 2026-01-30 | 048e786 |
| [src/process.rs](../../../crates/ork-executors/src/process.rs) | Local script execution and task status reporting. | 2026-01-30 | 2876883 |
