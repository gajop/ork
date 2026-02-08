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
| [Cargo.toml](../../../crates/ork-state/Cargo.toml) | Crate manifest and storage backend feature flags. | 2026-02-06 | 9d9b5dd |
| [build.rs](../../../crates/ork-state/build.rs) | Build script to rerun on migration changes. | 2026-02-06 | d4fff1b |
| [migrations/001_init.sql](../../../crates/ork-state/migrations/001_init.sql) | Initial schema for the legacy state store. | 2026-02-06 | d6ef2a5 |
| [migrations/002_add_executor_type.sql](../../../crates/ork-state/migrations/002_add_executor_type.sql) | Adds executor type metadata to workflows (legacy schema). | 2026-02-06 | 6f194bd |
| [migrations/003_add_indexes.sql](../../../crates/ork-state/migrations/003_add_indexes.sql) | Adds performance indexes for legacy queries. | 2026-02-06 | 7a49c8a |
| [migrations/004_rename_executor_agnostic.sql](../../../crates/ork-state/migrations/004_rename_executor_agnostic.sql) | Renames executor-specific columns in the legacy schema. | 2026-02-06 | 723370f |
| [migrations/005_add_dag_support.sql](../../../crates/ork-state/migrations/005_add_dag_support.sql) | Adds DAG workflow columns and task dependencies. | 2026-02-06 | f24808f |
| [migrations/006_workflow_tasks.sql](../../../crates/ork-state/migrations/006_workflow_tasks.sql) | Stores compiled DAGs and per-task executor type. | 2026-02-06 | 1662f24 |
| [migrations/007_add_task_logs.sql](../../../crates/ork-state/migrations/007_add_task_logs.sql) | Adds task log storage to the tasks table. | 2026-02-06 | 823943e |
| [migrations/008_add_task_retries_timeouts.sql](../../../crates/ork-state/migrations/008_add_task_retries_timeouts.sql) | Adds retry/timeout metadata to tasks. | 2026-02-06 | 6ceaaf9 |
| [migrations/009_add_workflow_schedules.sql](../../../crates/ork-state/migrations/009_add_workflow_schedules.sql) | Add workflow schedule columns and flags. | 2026-02-06 | 12f8574 |
| [migrations/010_add_pause_status.sql](../../../crates/ork-state/migrations/010_add_pause_status.sql) | Allow paused status for runs and tasks. | 2026-02-06 | 8a21120 |
| [migrations/011_add_task_signatures.sql](../../../crates/ork-state/migrations/011_add_task_signatures.sql) | Adds task signature metadata storage for workflow task introspection. | 2026-02-06 | 926e46b |
| [migrations/012_add_deferred_jobs.sql](../../../crates/ork-state/migrations/012_add_deferred_jobs.sql) | Adds deferred job tracking table and indexes for triggerer polling. | 2026-02-07 | c22c80f |
| [migrations_sqlite/001_init.sql](../../../crates/ork-state/migrations_sqlite/001_init.sql) | Standalone SQLite schema for local runs. | 2026-02-06 | 5ed599a |
| [migrations_sqlite/002_add_task_logs.sql](../../../crates/ork-state/migrations_sqlite/002_add_task_logs.sql) | Adds task log storage to the SQLite schema. | 2026-02-06 | 336fde8 |
| [migrations_sqlite/003_add_task_retries_timeouts.sql](../../../crates/ork-state/migrations_sqlite/003_add_task_retries_timeouts.sql) | Adds retry/timeout metadata to SQLite tasks. | 2026-02-06 | f326a0b |
| [migrations_sqlite/004_add_workflow_schedules.sql](../../../crates/ork-state/migrations_sqlite/004_add_workflow_schedules.sql) | SQLite schedule columns and flags. | 2026-02-06 | f0a496d |
| [migrations_sqlite/005_add_pause_status.sql](../../../crates/ork-state/migrations_sqlite/005_add_pause_status.sql) | SQLite paused status migration. | 2026-02-06 | bccc7cb |
| [migrations_sqlite/006_add_task_signatures.sql](../../../crates/ork-state/migrations_sqlite/006_add_task_signatures.sql) | SQLite migration for task signature metadata. | 2026-02-06 | a39905f |
| [migrations_sqlite/007_add_deferred_jobs.sql](../../../crates/ork-state/migrations_sqlite/007_add_deferred_jobs.sql) | Adds deferred job tracking table and indexes for SQLite mode. | 2026-02-07 | dd08bdd |
| [src/file.rs](../../../crates/ork-state/src/file.rs) | File-backed `StateStore` implementation for legacy flows. | 2026-02-07 | 9b801f8 |
| [src/file_database.rs](../../../crates/ork-state/src/file_database.rs) | `FileDatabase` backed by JSON files for local/dev use. | 2026-02-07 | 5986abc |
| [src/file_database/core.rs](../../../crates/ork-state/src/file_database/core.rs) | File-backed DB path helpers and JSON IO. | 2026-02-07 | eb5fd5b |
| [src/file_database/deferred_jobs.rs](../../../crates/ork-state/src/file_database/deferred_jobs.rs) | File-backed deferred job CRUD and completion tracking. | 2026-02-07 | 5055e56 |
| [src/file_database/runs.rs](../../../crates/ork-state/src/file_database/runs.rs) | File-backed run CRUD and status updates. | 2026-02-07 | 7b704a7 |
| [src/file_database/tasks.rs](../../../crates/ork-state/src/file_database/tasks.rs) | File-backed task CRUD, status updates, and logs. | 2026-02-07 | ce76f7e |
| [src/file_database/workflows.rs](../../../crates/ork-state/src/file_database/workflows.rs) | File-backed workflow CRUD and schedule updates. | 2026-02-07 | 5bc3591 |
| [src/lib.rs](../../../crates/ork-state/src/lib.rs) | Feature flags and re-exports for storage backends. | 2026-02-06 | a670611 |
| [src/memory.rs](../../../crates/ork-state/src/memory.rs) | In-memory `StateStore` implementation for legacy flows. | 2026-02-06 | e4a278b |
| [src/object_store.rs](../../../crates/ork-state/src/object_store.rs) | Object store trait and local implementation for specs/status/output. | 2026-02-06 | 4a1f6c3 |
| [src/postgres.rs](../../../crates/ork-state/src/postgres.rs) | `PostgresDatabase` implementation with SQLx queries and batch operations. | 2026-02-07 | dc45070 |
| [src/postgres/core.rs](../../../crates/ork-state/src/postgres/core.rs) | Postgres connection setup and migrations. | 2026-02-06 | 5314183 |
| [src/postgres/deferred_jobs.rs](../../../crates/ork-state/src/postgres/deferred_jobs.rs) | Postgres deferred job queries and status updates. | 2026-02-07 | 3ac4d50 |
| [src/postgres/runs.rs](../../../crates/ork-state/src/postgres/runs.rs) | Postgres run queries and status updates. | 2026-02-07 | 0df76ec |
| [src/postgres/tasks.rs](../../../crates/ork-state/src/postgres/tasks.rs) | Postgres task queries, updates, and dispatch filtering. | 2026-02-07 | ca6d1f8 |
| [src/postgres/workflows.rs](../../../crates/ork-state/src/postgres/workflows.rs) | Postgres workflow CRUD and schedule updates. | 2026-02-07 | a803602 |
| [src/sqlite.rs](../../../crates/ork-state/src/sqlite.rs) | `SqliteDatabase` implementation for standalone local runs. | 2026-02-07 | 5238ad3 |
| [src/sqlite/core.rs](../../../crates/ork-state/src/sqlite/core.rs) | SQLite connection setup and row mapping. | 2026-02-07 | 45a4498 |
| [src/sqlite/deferred_jobs.rs](../../../crates/ork-state/src/sqlite/deferred_jobs.rs) | SQLite deferred job queries and status updates. | 2026-02-07 | a4b1c11 |
| [src/sqlite/runs.rs](../../../crates/ork-state/src/sqlite/runs.rs) | SQLite run queries and status updates. | 2026-02-07 | dd2c246 |
| [src/sqlite/tasks.rs](../../../crates/ork-state/src/sqlite/tasks.rs) | SQLite task queries, updates, and dispatch filtering. | 2026-02-07 | 822131f |
| [src/sqlite/workflows.rs](../../../crates/ork-state/src/sqlite/workflows.rs) | SQLite workflow CRUD and schedule updates. | 2026-02-07 | 0f59d4a |
| [tests/postgres_backend_edge_test.rs](../../../crates/ork-state/tests/postgres_backend_edge_test.rs) | Additional Postgres integration edge-case coverage for scheduling, retries, and deferred job branches. | 2026-02-08 | 0000000 |
| [tests/postgres_backend_test.rs](../../../crates/ork-state/tests/postgres_backend_test.rs) | Postgres integration contract tests for workflow/run/task/deferred-job backend behavior. | 2026-02-08 | 0000000 |
| [tests/sqlite_backend_edge_test.rs](../../../crates/ork-state/tests/sqlite_backend_edge_test.rs) | SQLite integration edge-case coverage for pending-task accessors, retry metadata, outputs, and deferred-job cancellation. | 2026-02-08 | 0000000 |
