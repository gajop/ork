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
| [Cargo.toml](../../../crates/ork-web/Cargo.toml) | Crate manifest for the web UI/API. | 2026-02-06 | 478d441 |
| [src/api.rs](../../../crates/ork-web/src/api.rs) | HTTP routes and handlers for workflows, runs, and tasks. | 2026-02-07 | 6f93c7a |
| [src/api_helpers.rs](../../../crates/ork-web/src/api_helpers.rs) | Shared API helpers (time formatting, not-found detection). | 2026-02-06 | 84c8138 |
| [src/api_realtime.rs](../../../crates/ork-web/src/api_realtime.rs) | Websocket and UI handlers for realtime client updates. | 2026-02-07 | e392d0b |
| [src/api_routes.rs](../../../crates/ork-web/src/api_routes.rs) | API route handlers for runs, workflows, tasks, schedules, and websockets. | 2026-02-07 | d5f285a |
| [src/handlers.rs](../../../crates/ork-web/src/handlers.rs) | Workflow creation and run detail handlers with response structs. | 2026-02-07 | b034497 |
| [src/lib.rs](../../../crates/ork-web/src/lib.rs) | Library exports for the web crate. | 2026-02-07 | 85c00e7 |
| [src/main.rs](../../../crates/ork-web/src/main.rs) | Boots the Axum server and configures the app. | 2026-02-07 | 2eac514 |
| [src/workflow_tasks.rs](../../../crates/ork-web/src/workflow_tasks.rs) | Build workflow task rows from compiled workflows. | 2026-02-07 | 431f0c5 |
| [tests/api_endpoints_test.rs](../../../crates/ork-web/tests/api_endpoints_test.rs) | Integration tests for HTTP API endpoints. | 2026-02-07 | cac3273 |
| [ui/index.html](../../../crates/ork-web/ui/index.html) | Static HTML for the web UI. | 2026-02-06 | 752f20c |
