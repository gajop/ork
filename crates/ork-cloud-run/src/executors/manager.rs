use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{CloudRunClient, Executor, ProcessExecutor};
use crate::models::{ExecutorType, Workflow};

pub struct ExecutorManager {
    executors: Arc<RwLock<HashMap<Uuid, Arc<dyn Executor>>>>,
    process_executor: Arc<ProcessExecutor>,
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
            process_executor: Arc::new(ProcessExecutor::new(Some("test-scripts".to_string()))),
        }
    }

    pub async fn register_workflow(&self, workflow: &Workflow) -> Result<()> {
        let executor_type = ExecutorType::from_str(&workflow.executor_type).unwrap_or(ExecutorType::CloudRun);

        let executor: Arc<dyn Executor> = match executor_type {
            ExecutorType::Process => self.process_executor.clone(),
            ExecutorType::CloudRun => {
                let client = Arc::new(CloudRunClient::new(workflow.project.clone(), workflow.region.clone()).await?);
                client.clone().start_polling_task();
                client
            }
        };

        let mut executors = self.executors.write().await;
        executors.insert(workflow.id, executor);
        Ok(())
    }

    pub async fn get_executor(&self, workflow_id: Uuid) -> Result<Arc<dyn Executor>> {
        let executors = self.executors.read().await;
        executors
            .get(&workflow_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Executor not found for workflow {}", workflow_id))
    }
}
