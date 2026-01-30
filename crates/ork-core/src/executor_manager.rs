// ExecutorManager trait for managing executor instances
// Concrete implementation in ork-executors crate

#[cfg(feature = "async")]
use async_trait::async_trait;
#[cfg(feature = "async")]
use std::sync::Arc;
#[cfg(feature = "async")]
use uuid::Uuid;

#[cfg(feature = "async")]
use crate::executor::Executor;
#[cfg(feature = "async")]
use crate::models_v2::Workflow;

/// ExecutorManager trait for managing workflow executors
///
/// This trait abstracts the management of executor instances.
/// Different implementations can handle different executor backends.
#[cfg(feature = "async")]
#[async_trait]
pub trait ExecutorManager: Send + Sync {
    /// Register a workflow and create its executor
    async fn register_workflow(&self, workflow: &Workflow) -> anyhow::Result<()>;

    /// Get the executor for a workflow
    async fn get_executor(&self, workflow_id: Uuid) -> anyhow::Result<Arc<dyn Executor>>;
}
