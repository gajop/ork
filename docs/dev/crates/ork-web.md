# ork-web

Axum-based web UI/API backed by the primary database. It reads workflows/runs/tasks and triggers new runs against the DB-backed scheduler.

## Owns

- Web API routes for listing workflows, runs, and tasks
- Run trigger endpoint that creates `runs` rows in Postgres
- Web server bootstrapping

## Notes

- Does not run the scheduler loop; it expects `ork run` (or another scheduler host) to be running.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-web/Cargo.toml) | Crate manifest for the web UI/API. | 2026-01-31 | cf1ea18 |
| [src/api.rs](../../../crates/ork-web/src/api.rs) | HTTP routes and handlers for workflows, runs, and tasks. | 2026-01-31 | ea6a033 |
| [src/main.rs](../../../crates/ork-web/src/main.rs) | Boots the Axum server and configures the app. | 2026-01-31 | d229db8 |
| [ui/index.html](../../../crates/ork-web/ui/index.html) | Static HTML for the web UI. | 2026-01-31 | b208379 |
