# ork-web (legacy)

Axum-based web UI/API. Currently uses [`ork-runner::LocalScheduler`](../../../crates/ork-runner/src/scheduler.rs) and legacy state store types.

## Owns

- Web API routes for viewing workflows, runs, and tasks
- Web server bootstrapping

## Migration Note

- Should be migrated to [`ork-core::Scheduler`](../../../crates/ork-core/src/scheduler.rs) + [`ork-state::Database`](../../../crates/ork-core/src/database.rs).

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-web/Cargo.toml) | Crate manifest for the web UI/API. | 2026-01-25 | fe6c43c |
| [src/api.rs](../../../crates/ork-web/src/api.rs) | HTTP routes and handlers for workflows, runs, and tasks. | 2026-01-30 | 44e74dc |
| [src/main.rs](../../../crates/ork-web/src/main.rs) | Boots the Axum server and configures the app. | 2026-01-30 | 4e9032f |
| [ui/index.html](../../../crates/ork-web/ui/index.html) | Static HTML for the legacy web UI. | 2026-01-25 | e12f841 |
