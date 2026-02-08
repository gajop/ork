# ork-web

Axum-based web UI/API backed by the primary database. It reads workflows/runs/tasks and triggers new runs against the DB-backed scheduler.

## Owns

- Web API routes for listing workflows, runs, and tasks
- Run trigger endpoint that creates `runs` rows in Postgres
- Workflow creation endpoint that accepts YAML and compiles DAGs
- Web server bootstrapping

## Notes

- Does not run the scheduler loop; it expects `ork run` (or another scheduler host) to be running.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-web/Cargo.toml) | Crate manifest for the web UI/API. | 2026-02-08 | d9da3f4 |
| [src/api.rs](../../../crates/ork-web/src/api.rs) | HTTP routes and handlers for workflows, runs, and tasks. | 2026-02-08 | 6f93c7a |
| [src/api_helpers.rs](../../../crates/ork-web/src/api_helpers.rs) | Shared API helpers (time formatting, not-found detection). | 2026-02-06 | 84c8138 |
| [src/api_realtime.rs](../../../crates/ork-web/src/api_realtime.rs) | Websocket and UI handlers for realtime client updates. | 2026-02-08 | c95554d |
| [src/api_routes.rs](../../../crates/ork-web/src/api_routes.rs) | API route handlers for runs, workflows, tasks, schedules, and websockets. | 2026-02-08 | cb4b542 |
| [src/handlers.rs](../../../crates/ork-web/src/handlers.rs) | Workflow creation and run detail handlers with response structs. | 2026-02-07 | 9e706f9 |
| [src/lib.rs](../../../crates/ork-web/src/lib.rs) | Library exports for the web crate. | 2026-02-07 | 85c00e7 |
| [src/main.rs](../../../crates/ork-web/src/main.rs) | Boots the Axum server and configures the app. | 2026-02-08 | b7f6877 |
| [src/workflow_tasks.rs](../../../crates/ork-web/src/workflow_tasks.rs) | Build workflow task rows from compiled workflows. | 2026-02-08 | 8ae3c4e |
| [tests/api_db_error_paths_test.rs](../../../crates/ork-web/tests/api_db_error_paths_test.rs) | Integration tests for DB failure/error branches across API handlers. | 2026-02-08 | 5313253 |
| [tests/api_endpoints_test.rs](../../../crates/ork-web/tests/api_endpoints_test.rs) | Integration tests for HTTP API endpoints. | 2026-02-08 | d9c31ed |
| [tests/api_error_paths_test.rs](../../../crates/ork-web/tests/api_error_paths_test.rs) | Integration tests for API error, conflict, and invalid-input paths. | 2026-02-08 | 385c800 |
| [tests/api_realtime_test.rs](../../../crates/ork-web/tests/api_realtime_test.rs) | Integration tests for realtime websocket broadcasts and API server startup serving the UI. | 2026-02-08 | 8c35471 |
| [ui/index.html](../../../crates/ork-web/ui/index.html) | Static HTML for the web UI. | 2026-02-06 | 752f20c |
