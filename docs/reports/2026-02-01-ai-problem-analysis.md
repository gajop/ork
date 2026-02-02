# AI Problem Analysis (2026-02-01)

Scope: current repo state as of 2026-02-01. This report is condensed to a short, basic-functionality-first priority list.

## Top 10 basic functionality priorities

1. **Task heartbeats + stuck detection**: detect dead executors and recover `dispatched`/`running` tasks; add consistent cancellation for running tasks (Cloud Run + process).
2. **Run idempotency + dedup**: prevent accidental duplicate run creation (API/CLI/schedules) with idempotency keys or unique constraints.
3. **FileDatabase parity**: implement `get_pending_tasks_with_workflow` so the scheduler runs against the file backend.
4. **Dependency failure + retry semantics**: add partial retry/skip semantics (not just “fail downstream”).
5. **Retry policy tuning**: add jitter and per-task backoff policy configuration.
6. **Output + artifact model**: standardize task output capture, add artifact retention, and improve upstream injection semantics.
7. **YAML schema validation**: typed schema for tasks/executors and stricter validation of params.
8. **Workflow versioning + auditability**: preserve definitions per run and record who/what changed or triggered runs.
9. **Concurrency controls**: per-workflow/run limits in addition to executor dispatch caps.
10. **Local executor safety**: mitigate `sh -c` injection; add resource limits and sandboxing for process tasks.

## Recently addressed

- Pause/resume for runs and tasks.
- Websocket-driven UI updates and API pagination/filtering.

## Notes

If you want the older categorical breakdown restored, we can regenerate it from current code/state.
