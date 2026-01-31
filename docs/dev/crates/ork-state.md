# ork-state

Storage backends for Ork. Implements the [`ork-core::Database`](../../../crates/ork-core/src/database.rs) trait.

## Implementations

- `PostgresDatabase`: primary production backend (sqlx)
- `FileDatabase`: file-backed JSON storage for local/dev

## Notes

- Postgres schema and migrations live in [crates/ork-state/migrations/](../../../crates/ork-state/migrations/) (mirrored in [crates/ork-cli/](../../../crates/ork-cli/)).
- Feature flags include `postgres`, `sqlite`, and `file`. Only Postgres and file are currently implemented.

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-state/Cargo.toml) | Crate manifest and storage backend feature flags. | 2026-01-30 | 7cdaafe |
| [migrations/001_init.sql](../../../crates/ork-state/migrations/001_init.sql) | Initial schema for the legacy state store. | 2026-01-30 | d6ef2a5 |
| [migrations/002_add_executor_type.sql](../../../crates/ork-state/migrations/002_add_executor_type.sql) | Adds executor type metadata to workflows (legacy schema). | 2026-01-30 | 6f194bd |
| [migrations/003_add_indexes.sql](../../../crates/ork-state/migrations/003_add_indexes.sql) | Adds performance indexes for legacy queries. | 2026-01-30 | 7a49c8a |
| [migrations/004_rename_executor_agnostic.sql](../../../crates/ork-state/migrations/004_rename_executor_agnostic.sql) | Renames executor-specific columns in the legacy schema. | 2026-01-30 | 723370f |
| [migrations/005_add_dag_support.sql](../../../crates/ork-state/migrations/005_add_dag_support.sql) | Adds DAG workflow columns and task dependencies. | 2026-01-31 | f24808f |
| [migrations/006_workflow_tasks.sql](../../../crates/ork-state/migrations/006_workflow_tasks.sql) | Stores compiled DAGs and per-task executor type. | 2026-01-31 | 1662f24 |
| [src/file.rs](../../../crates/ork-state/src/file.rs) | File-backed `StateStore` implementation for legacy flows. | 2026-01-30 | 789fd47 |
| [src/file_database.rs](../../../crates/ork-state/src/file_database.rs) | `FileDatabase` backed by JSON files for local/dev use. | 2026-01-31 | e23d636 |
| [src/lib.rs](../../../crates/ork-state/src/lib.rs) | Feature flags and re-exports for storage backends. | 2026-01-30 | cfc96f1 |
| [src/memory.rs](../../../crates/ork-state/src/memory.rs) | In-memory `StateStore` implementation for legacy flows. | 2026-01-30 | e4a278b |
| [src/object_store.rs](../../../crates/ork-state/src/object_store.rs) | Object store trait and local implementation for specs/status/output. | 2026-01-30 | 4a1f6c3 |
| [src/postgres.rs](../../../crates/ork-state/src/postgres.rs) | `PostgresDatabase` implementation with SQLx queries and batch operations. | 2026-01-31 | 750ff66 |
