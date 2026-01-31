# Crate Structure

```
ork/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── ork-cli/                  # Main binary (ork command)
│   ├── ork-core/                 # Core traits and models
│   ├── ork-state/                # Database implementations
│   ├── ork-executors/            # Executor implementations
│   ├── ork-runner/               # Legacy task runner (deprecated)
│   └── ork-web/                  # Web UI
└── docs/                         # Documentation

```

## ork-cli

Main CLI binary. Produces the `ork` executable.

**Responsibilities:**
- CLI commands (create-workflow, trigger, status, etc.)
- Database initialization and migrations
- Scheduler instantiation and execution
- Performance testing binary

**Dependencies:**
- ork-core (Database, Scheduler, ExecutorManager traits)
- ork-state (PostgresDatabase)
- ork-executors (ExecutorManager implementation)
- sqlx (migrations)
- clap (CLI parsing)
- tokio (async runtime)

**Key files:**
- `src/main.rs` - CLI entry point
- `src/bin/perf-test.rs` - Performance testing binary
- `migrations/` - SQL migration files
- `perf-configs/` - Performance test configurations

## ork-core

Core domain models, traits, and orchestration logic.

**Exports:**
```rust
// Models
pub mod models {
    pub struct Workflow { id, name, job_name, executor_type, ... }
    pub struct Run { id, workflow_id, status, ... }
    pub struct Task { id, run_id, task_index, status, ... }
    pub struct TaskWithWorkflow { ... }  // JOIN result

    pub enum RunStatus { Pending, Running, Success, Failed }
    pub enum TaskStatus { Pending, Dispatched, Running, Success, Failed }
}

// Traits
pub mod database {
    #[async_trait]
    pub trait Database: Send + Sync {
        async fn create_workflow(...) -> Result<Workflow>;
        async fn create_run(...) -> Result<Run>;
        async fn batch_create_tasks(...) -> Result<()>;
        async fn get_pending_tasks_with_workflow(...) -> Result<Vec<TaskWithWorkflow>>;
        async fn batch_update_task_status(...) -> Result<()>;
        // ... more methods
    }
}

pub mod executor {
    #[async_trait]
    pub trait Executor: Send + Sync {
        async fn execute(&self, task_id, job_name, params) -> Result<String>;
        async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>);
    }

    pub struct StatusUpdate {
        pub task_id: Uuid,
        pub status: String,
    }
}

pub mod executor_manager {
    #[async_trait]
    pub trait ExecutorManager: Send + Sync {
        async fn register_workflow(&self, workflow: &Workflow) -> Result<()>;
        async fn get_executor(&self, workflow_id: Uuid) -> Result<Arc<dyn Executor>>;
    }
}

pub mod scheduler {
    pub struct Scheduler<D: Database, E: ExecutorManager> {
        db: Arc<D>,
        executor_manager: Arc<E>,
        config: OrchestratorConfig,
    }

    impl Scheduler {
        pub async fn run(&self) -> Result<()>;
    }
}

pub mod config {
    pub struct OrchestratorConfig {
        pub poll_interval_secs: f64,
        pub max_tasks_per_batch: i64,
        pub max_concurrent_dispatches: usize,
        pub max_concurrent_status_checks: usize,
    }
}
```

**Dependencies:**
- async-trait
- tokio
- anyhow
- futures
- tracing
- serde, serde_json
- chrono
- uuid
- sqlx (optional, for models)

**No dependencies on:**
- Specific database implementations
- Specific executor implementations
- CLI or API code

## ork-state

Database trait implementations.

**Exports:**
```rust
#[cfg(feature = "postgres")]
pub struct PostgresDatabase {
    pool: sqlx::PgPool,
}

#[cfg(feature = "file")]
pub struct FileDatabase {
    base: PathBuf,
}

// Legacy (to be deprecated)
pub struct FileStateStore { ... }
pub struct InMemoryStateStore { ... }
pub trait StateStore { ... }
```

**Features:**
- `postgres` - PostgresDatabase implementation (default)
- `sqlite` - SqliteDatabase implementation (planned)
- `file` - FileDatabase (JSON file storage)

**PostgresDatabase:**
- Uses sqlx connection pool
- Prepared statements for all queries
- Batch operations use transactions
- Composite indexes for performance

**FileDatabase:**
- JSON files in `{base}/workflows/`, `{base}/runs/`, `{base}/tasks/`
- No indexes (file-based)
- Uses serde_json for (de)serialization
- Good for development/testing

## ork-executors

Executor trait implementations and manager.

**Exports:**
```rust
pub struct ExecutorManager {
    executors: HashMap<Uuid, Arc<dyn Executor>>,
}

#[cfg(feature = "process")]
pub struct ProcessExecutor {
    workflow_id: Uuid,
    script_dir: PathBuf,
}

#[cfg(feature = "cloudrun")]
pub struct CloudRunExecutor {
    workflow_id: Uuid,
    job_name: String,
    project: String,
    region: String,
}
```

**Features:**
- `process` - Local process executor (default)
- `cloudrun` - Google Cloud Run executor

**ProcessExecutor:**
- Spawns local processes via `tokio::process::Command`
- Monitors process completion via async tasks
- Sends status updates via channel
- Script path: `{script_dir}/{job_name}`

**CloudRunExecutor:**
- Creates Cloud Run jobs via GCP API
- Polls for status periodically
- Sends status updates via channel
- Requires GCP authentication

**ExecutorManager:**
- Maintains map of workflow_id → executor
- Creates executor based on workflow.executor_type
- Thread-safe (Arc + Mutex)

## ork-runner (Legacy - Deprecated)

Old task runner. To be removed once fully migrated to new executor system.

## ork-web

Web UI for visualizing runs and tasks. Currently uses legacy StateStore trait.

**Status:** Needs migration to use Database trait instead of StateStore.

## Dependency Graph

```
ork-cli
  ├── ork-core (Database, Scheduler, ExecutorManager traits)
  ├── ork-state (PostgresDatabase)
  └── ork-executors (ExecutorManager, ProcessExecutor, CloudRunExecutor)

ork-state
  └── ork-core (models)

ork-executors
  └── ork-core (Executor trait, models)

ork-web
  ├── ork-core (legacy types)
  ├── ork-state (StateStore - needs migration)
  └── ork-runner (legacy - needs migration)
```

## Build Configuration

**Workspace Cargo.toml:**
```toml
[workspace]
members = [
    "crates/ork-cli",
    "crates/ork-web",
    "crates/ork-core",
    "crates/ork-state",
    "crates/ork-runner",
    "crates/ork-executors",
]
```

**Feature flags:**
- ork-cli: `postgres` (default), `process` (default), `cloudrun`, `sqlite`
- ork-state: `postgres`, `sqlite`, `file`
- ork-executors: `process`, `cloudrun`

**Binaries:**
- `ork` - Main CLI (from ork-cli)
- `perf-test` - Performance testing (from ork-cli)
