# ork-state

Storage backends for Ork. Implements the [`ork-core::Database`](../../../crates/ork-core/src/database.rs) trait.

## Implementations

- `PostgresDatabase`: primary production backend (sqlx)
- `SqliteDatabase`: standalone local backend (sqlx, single-file DB)
- `FileDatabase`: file-backed JSON storage for local/dev

## Notes

- Postgres schema and migrations live in [crates/ork-state/migrations/](../../../crates/ork-state/migrations/) (mirrored in [crates/ork-cli/](../../../crates/ork-cli/)).
- SQLite schema lives in [crates/ork-state/migrations_sqlite/](../../../crates/ork-state/migrations_sqlite/).
- Feature flags include `postgres`, `sqlite`, and `file`.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-state/Cargo.toml) | Crate manifest and storage backend feature flags. | 2026-01-31 | 9d9b5dd |
| [build.rs](../../../crates/ork-state/build.rs) | Build script to rerun on migration changes. | 2026-02-01 | d4fff1b |
| [migrations/001_init.sql](../../../crates/ork-state/migrations/001_init.sql) | Initial schema for the legacy state store. | 2026-01-30 | d6ef2a5 |
| [migrations/002_add_executor_type.sql](../../../crates/ork-state/migrations/002_add_executor_type.sql) | Adds executor type metadata to workflows (legacy schema). | 2026-01-30 | 6f194bd |
| [migrations/003_add_indexes.sql](../../../crates/ork-state/migrations/003_add_indexes.sql) | Adds performance indexes for legacy queries. | 2026-01-30 | 7a49c8a |
| [migrations/004_rename_executor_agnostic.sql](../../../crates/ork-state/migrations/004_rename_executor_agnostic.sql) | Renames executor-specific columns in the legacy schema. | 2026-01-30 | 723370f |
| [migrations/005_add_dag_support.sql](../../../crates/ork-state/migrations/005_add_dag_support.sql) | Adds DAG workflow columns and task dependencies. | 2026-01-31 | f24808f |
| [migrations/006_workflow_tasks.sql](../../../crates/ork-state/migrations/006_workflow_tasks.sql) | Stores compiled DAGs and per-task executor type. | 2026-01-31 | 1662f24 |
| [migrations/007_add_task_logs.sql](../../../crates/ork-state/migrations/007_add_task_logs.sql) | Adds task log storage to the tasks table. | 2026-01-31 | 823943e |
| [migrations/008_add_task_retries_timeouts.sql](../../../crates/ork-state/migrations/008_add_task_retries_timeouts.sql) | Adds retry/timeout metadata to tasks. | 2026-02-01 | 6ceaaf9 |
| [migrations/009_add_workflow_schedules.sql](../../../crates/ork-state/migrations/009_add_workflow_schedules.sql) | Add workflow schedule columns and flags. | 2026-02-01 | 12f8574 |
| [migrations/010_add_pause_status.sql](../../../crates/ork-state/migrations/010_add_pause_status.sql) | Allow paused status for runs and tasks. | 2026-02-01 | 8a21120 |
| [migrations_sqlite/001_init.sql](../../../crates/ork-state/migrations_sqlite/001_init.sql) | Standalone SQLite schema for local runs. | 2026-01-31 | 5ed599a |
| [migrations_sqlite/002_add_task_logs.sql](../../../crates/ork-state/migrations_sqlite/002_add_task_logs.sql) | Adds task log storage to the SQLite schema. | 2026-01-31 | 336fde8 |
| [migrations_sqlite/003_add_task_retries_timeouts.sql](../../../crates/ork-state/migrations_sqlite/003_add_task_retries_timeouts.sql) | Adds retry/timeout metadata to SQLite tasks. | 2026-02-01 | f326a0b |
| [migrations_sqlite/004_add_workflow_schedules.sql](../../../crates/ork-state/migrations_sqlite/004_add_workflow_schedules.sql) | SQLite schedule columns and flags. | 2026-02-01 | f0a496d |
| [migrations_sqlite/005_add_pause_status.sql](../../../crates/ork-state/migrations_sqlite/005_add_pause_status.sql) | SQLite paused status migration. | 2026-02-01 | bccc7cb |
| [src/file.rs](../../../crates/ork-state/src/file.rs) | File-backed `StateStore` implementation for legacy flows. | 2026-01-30 | 789fd47 |
| [src/file_database.rs](../../../crates/ork-state/src/file_database.rs) | `FileDatabase` backed by JSON files for local/dev use. | 2026-02-01 | 506a20a |
| [src/file_database/core.rs](../../../crates/ork-state/src/file_database/core.rs) | File-backed DB path helpers and JSON IO. | 2026-02-01 | 03f5395 |
| [src/file_database/runs.rs](../../../crates/ork-state/src/file_database/runs.rs) | File-backed run CRUD and status updates. | 2026-02-01 | fc09fcd |
| [src/file_database/tasks.rs](../../../crates/ork-state/src/file_database/tasks.rs) | File-backed task CRUD, status updates, and logs. | 2026-02-01 | e3a8821 |
| [src/file_database/workflows.rs](../../../crates/ork-state/src/file_database/workflows.rs) | File-backed workflow CRUD and schedule updates. | 2026-02-01 | ca567e3 |
| [src/lib.rs](../../../crates/ork-state/src/lib.rs) | Feature flags and re-exports for storage backends. | 2026-01-31 | a670611 |
| [src/memory.rs](../../../crates/ork-state/src/memory.rs) | In-memory `StateStore` implementation for legacy flows. | 2026-01-30 | e4a278b |
| [src/object_store.rs](../../../crates/ork-state/src/object_store.rs) | Object store trait and local implementation for specs/status/output. | 2026-01-30 | 4a1f6c3 |
| [src/postgres.rs](../../../crates/ork-state/src/postgres.rs) | `PostgresDatabase` implementation with SQLx queries and batch operations. | 2026-02-01 | 9a1caf5 |
| [src/postgres/core.rs](../../../crates/ork-state/src/postgres/core.rs) | Postgres connection setup and migrations. | 2026-02-01 | 5314183 |
| [src/postgres/runs.rs](../../../crates/ork-state/src/postgres/runs.rs) | Postgres run queries and status updates. | 2026-02-01 | 7bebb6c |
| [src/postgres/tasks.rs](../../../crates/ork-state/src/postgres/tasks.rs) | Postgres task queries, updates, and dispatch filtering. | 2026-02-01 | 9028b76 |
| [src/postgres/workflows.rs](../../../crates/ork-state/src/postgres/workflows.rs) | Postgres workflow CRUD and schedule updates. | 2026-02-01 | ad7a88e |
| [src/sqlite.rs](../../../crates/ork-state/src/sqlite.rs) | `SqliteDatabase` implementation for standalone local runs. | 2026-02-01 | 83578c3 |
| [src/sqlite/core.rs](../../../crates/ork-state/src/sqlite/core.rs) | SQLite connection setup and row mapping. | 2026-02-01 | d89489d |
| [src/sqlite/runs.rs](../../../crates/ork-state/src/sqlite/runs.rs) | SQLite run queries and status updates. | 2026-02-01 | 439a54b |
| [src/sqlite/tasks.rs](../../../crates/ork-state/src/sqlite/tasks.rs) | SQLite task queries, updates, and dispatch filtering. | 2026-02-01 | d32232a |
| [src/sqlite/workflows.rs](../../../crates/ork-state/src/sqlite/workflows.rs) | SQLite workflow CRUD and schedule updates. | 2026-02-01 | cd814f6 |
