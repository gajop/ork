// ExecutorManager trait for managing executor instances
// Concrete implementation in ork-executors crate

use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::executor::Executor;
use crate::models::Workflow;

/// ExecutorManager trait for managing workflow executors
///
/// This trait abstracts the management of executor instances.
/// Different implementations can handle different executor backends.
#[async_trait]
pub trait ExecutorManager: Send + Sync {
    /// Register a workflow and create its executor
    async fn register_workflow(&self, workflow: &Workflow) -> anyhow::Result<()>;

    /// Get the executor for a workflow
    async fn get_executor(&self, workflow_id: Uuid) -> anyhow::Result<Arc<dyn Executor>>;
}
