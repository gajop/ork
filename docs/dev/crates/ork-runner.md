# ork-runner (legacy)

Legacy scheduler and execution path. It is no longer used by ork-web and is retained for reference while the new DB-backed scheduler is adopted everywhere.

## Owns

- [`LocalScheduler`](../../../crates/ork-runner/src/scheduler.rs) and legacy executor wiring
- Legacy state store integration

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-runner/Cargo.toml) | Crate manifest for the legacy runner. | 2026-01-31 | 68acd69 |
| [src/executor.rs](../../../crates/ork-runner/src/executor.rs) | Legacy executor wiring used by the local runner. | 2026-01-31 | 9f94488 |
| [src/lib.rs](../../../crates/ork-runner/src/lib.rs) | Re-exports runner components and module structure. | 2026-01-30 | 6f13cce |
| [src/runner/helpers.rs](../../../crates/ork-runner/src/runner/helpers.rs) | Helper utilities for task dispatch and failure handling. | 2026-01-30 | 1544871 |
| [src/runner/mod.rs](../../../crates/ork-runner/src/runner/mod.rs) | Runner module wiring and re-exports. | 2026-01-30 | 20df2e6 |
| [src/runner/run.rs](../../../crates/ork-runner/src/runner/run.rs) | Core workflow execution loop for the legacy runner. | 2026-01-30 | 980514d |
| [src/runner/types.rs](../../../crates/ork-runner/src/runner/types.rs) | Local runner types like `RunSummary` and task state. | 2026-01-30 | f67b97a |
| [src/scheduler.rs](../../../crates/ork-runner/src/scheduler.rs) | Legacy `LocalScheduler` loop. | 2026-01-30 | 6d42c4a |
