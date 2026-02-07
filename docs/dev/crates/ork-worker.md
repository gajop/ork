# ork-worker

Worker process binary for compile/execute paths used by isolated execution deployments.

## Owns

- Worker CLI entrypoint
- Task compilation endpoint/runtime
- Task execution endpoint/runtime

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-worker/Cargo.toml) | Crate manifest and worker feature/runtime dependencies. | 2026-02-07 | ef5c10a |
| [src/compile.rs](../../../crates/ork-worker/src/compile.rs) | Worker-side workflow/task compilation handlers. | 2026-02-07 | e4d6154 |
| [src/execute.rs](../../../crates/ork-worker/src/execute.rs) | Worker-side task execution handlers and status reporting. | 2026-02-07 | 5d32726 |
| [src/main.rs](../../../crates/ork-worker/src/main.rs) | Worker process entrypoint and HTTP/IPC wiring. | 2026-02-07 | 8478d26 |
