mod cloud_run;
mod manager;
mod process;

pub use cloud_run::CloudRunClient;
pub use manager::ExecutorManager;
pub use process::ProcessExecutor;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Status update event from executor to scheduler
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub task_id: Uuid,
    pub status: String,
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, task_id: Uuid, job_name: &str, params: Option<serde_json::Value>) -> Result<String>;

    /// Set the status update channel for event-driven notifications
    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>);
}
