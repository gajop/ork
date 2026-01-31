# Database Schema

Ork uses a relational database (PostgreSQL or SQLite) for all state management.

## Core Tables

- **workflows** - Workflow definitions with executor configuration
- **runs** - Workflow execution instances
- **tasks** - Individual task executions within a run

## Schema Source

The complete canonical schema showing the final state after all migrations is in [crates/ork-cli/schema.sql](../../crates/ork-cli/schema.sql). This file is auto-generated from the database using `just dump-schema`.

For the migration history that produces this schema, see [crates/ork-cli/migrations/](../../crates/ork-cli/migrations/):

- [001_init.sql](../../crates/ork-cli/migrations/001_init.sql) - Initial tables and indexes
- [002_add_executor_type.sql](../../crates/ork-cli/migrations/002_add_executor_type.sql) - Executor type column
- [003_add_indexes.sql](../../crates/ork-cli/migrations/003_add_indexes.sql) - Composite indexes for query optimization
- [004_rename_executor_agnostic.sql](../../crates/ork-cli/migrations/004_rename_executor_agnostic.sql) - Rename to executor-agnostic names

Migrations are applied automatically via `ork init` using sqlx.

## Models

The Rust models that map to these tables are in [crates/ork-core/src/models.rs](../../crates/ork-core/src/models.rs):

- `Workflow` - Maps to workflows table
- `Run` - Maps to runs table
- `Task` - Maps to tasks table
- `TaskWithWorkflow` - JOIN result for optimized queries

## Database Implementations

Multiple database backends via the `Database` trait:

### PostgresDatabase

Implementation: [crates/ork-state/src/postgres.rs](../../crates/ork-state/src/postgres.rs)

- Uses sqlx connection pool
- Batch operations with transactions
- Composite indexes for performance

### FileDatabase

Implementation: [crates/ork-state/src/file_database.rs](../../crates/ork-state/src/file_database.rs)

- JSON file storage: `{base}/workflows/`, `{base}/runs/`, `{base}/tasks/`
- Uses serde_json for serialization
- No indexes (file-based, not query-optimized)
- Good for development/testing

## Query Patterns

The key performance optimization is using JOIN queries to avoid N+1 problems. See `get_pending_tasks_with_workflow()` in the Database trait implementations for examples.
