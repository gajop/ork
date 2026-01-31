// Legacy state stores (to be deprecated - use Database trait instead)
mod file;
mod memory;
pub mod object_store;

pub use file::FileStateStore;
pub use memory::InMemoryStateStore;
pub use object_store::{LocalObjectStore, ObjectStore};

// Database trait implementations
#[cfg(feature = "file")]
pub mod file_database;

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "sqlite")]
pub mod sqlite;

#[cfg(feature = "file")]
pub use file_database::FileDatabase;

#[cfg(feature = "postgres")]
pub use postgres::PostgresDatabase;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteDatabase;

use async_trait::async_trait;
use ork_core::error::OrkResult;
use ork_core::types::{Run, RunId, RunStatus, TaskRun};
use ork_core::workflow::Workflow;

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn create_run(&self, workflow: &Workflow) -> OrkResult<Run>;
    async fn get_run(&self, run_id: &RunId) -> OrkResult<Option<Run>>;
    async fn list_runs(&self) -> OrkResult<Vec<Run>>;
    async fn list_task_runs(&self, run_id: &RunId) -> OrkResult<Vec<TaskRun>>;
    async fn upsert_task_run(&self, task_run: TaskRun) -> OrkResult<()>;
    async fn update_run_status(&self, run_id: &RunId, status: RunStatus) -> OrkResult<()>;
}
