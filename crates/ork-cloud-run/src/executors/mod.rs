mod cloud_run;
mod manager;
mod process;

pub use cloud_run::CloudRunClient;
pub use manager::ExecutorManager;
pub use process::ProcessExecutor;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, job_name: &str, params: Option<serde_json::Value>) -> Result<String>;
    async fn get_status(&self, execution_id: &str) -> Result<String>;
}
