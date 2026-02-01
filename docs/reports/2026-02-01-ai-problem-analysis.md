# AI Problem Analysis (2026-02-01)

Scope: current repo state as of 2026-02-01. This is a focused outline of known problems/limitations (design + implementation) to guide near-term fixes.

## Core product gaps

- No retries, timeouts, or backoff enforcement (fields exist in YAML but are not acted on by the scheduler/executors).
- No heartbeats or stuck-task detection; tasks can remain `dispatched`/`running` forever if an executor dies.
- Output passing is minimal: only JSON stdout is captured as task output, and upstream outputs are exposed via `ORK_UPSTREAM_JSON` (auto-injected only when task input is empty).
- No cancellation/resume/pausing for runs or tasks (cancel exists in schema but not wired).
- No scheduling/cron triggers; all runs are manual.
- No multi-tenant controls, auth, or RBAC. API/UI are open.

## Workflow & DAG model limitations

- Workflow/task versioning is not preserved; updating a workflow replaces `workflow_tasks` and loses previous definitions.
- The workflow-level fields (job_name/executor_type) are largely bypassed by per-task executor metadata, which creates confusing split ownership.
- YAML schema is permissive and untyped (no strong validation of task params or executor-specific settings).
- Dynamic DAGs and parameter expansion are not supported; DAGs are fully static.
- Root/path handling for Python modules relies on local filesystem paths, which is fragile for containerized or remote execution.

## Scheduler/runtime risks

- Single scheduler instance; no HA, leader election, or leasing.
- Polling-only loop; no event-driven scheduling or push-based task state.
- No idempotency or deduplication on run creation.
- Dependency failure propagation is “fail downstream” only; no partial retry or skip semantics.
- No concurrency controls at run/workflow granularity (only executor-level dispatch limits).

## Storage/data layer issues

- `FileDatabase` does not implement `get_pending_tasks_with_workflow`, so the scheduler cannot run against it.
- SQLite dependency resolution is done in application memory and scans all pending tasks; no SQL-level filtering or paging.
- Logs are stored as a single growing string in `tasks.logs` with no truncation/retention policy.
- No schema-level constraints for JSON fields (params/output), making data integrity hard to enforce.

## Executor design limitations

- “Python” is not a separate executor; it is handled by `ProcessExecutor` with special params, which blurs separation of concerns.
- Process executor uses `sh -c`, which is not portable to Windows and is vulnerable to command injection if inputs are not trusted.
- No resource limits (CPU/memory), sandboxing, or isolation for local process tasks.
- No standardized task output capture; stdout is treated as logs only.
- Cloud Run executor lacks log streaming and cancellation support.

## API/CLI boundary issues

- CLI mixes direct DB access with REST API calls (e.g., `run-workflow` uses API, while other commands require DB access).
- API has no pagination, filtering, or auth; list endpoints are coarse and unbounded.
- `POST /api/workflows` can update tasks but cannot update workflow metadata (project/region/job) once created.
- No API endpoint for run cancellation or retry.

## UI limitations

- UI is polling-based only; no websocket streaming.
- No pagination or filters for workflows/runs; large deployments will be slow/noisy.
- Task logs are shown as a single blob; no tailing controls or severity filtering.
- Graph layout is basic and does not scale for large DAGs.

## Build/packaging & dev experience

- Docker build context is the repo root; without a root `.dockerignore`, builds can be slow and invalidated often.
- No automated integration tests for scheduler + API + UI end-to-end behavior.
- Local dev defaults assume Postgres or SQLite running locally; configuration for remote environments is not centralized.

## Observability & operations gaps

- No metrics export (Prometheus/OpenTelemetry) beyond log lines.
- No audit trail for who/what triggered runs.
- No structured log ingestion or correlation IDs across scheduler, executors, and web UI.
