use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use ork_core::executor::Executor;
use ork_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use ork_core::models_v2::{ExecutorType, Workflow};

#[cfg(feature = "process")]
use crate::process::ProcessExecutor;
#[cfg(feature = "cloudrun")]
use crate::cloud_run::CloudRunClient;

pub struct ExecutorManager {
    executors: Arc<RwLock<HashMap<Uuid, Arc<dyn Executor>>>>,
    #[cfg(feature = "process")]
    process_executor: Arc<ProcessExecutor>,
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            executors: Arc::new(RwLock::new(HashMap::new())),
            #[cfg(feature = "process")]
            process_executor: Arc::new(ProcessExecutor::new(Some("test-scripts".to_string()))),
        }
    }
}

#[async_trait]
impl ExecutorManagerTrait for ExecutorManager {
    async fn register_workflow(&self, workflow: &Workflow) -> Result<()> {
        let executor_type = ExecutorType::from_str(&workflow.executor_type).unwrap_or(ExecutorType::CloudRun);

        let executor: Arc<dyn Executor> = match executor_type {
            #[cfg(feature = "process")]
            ExecutorType::Process => self.process_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Process => {
                anyhow::bail!("Process executor not enabled. Enable the 'process' feature flag.");
            }
            #[cfg(feature = "cloudrun")]
            ExecutorType::CloudRun => {
                let client = Arc::new(CloudRunClient::new(workflow.project.clone(), workflow.region.clone()).await?);
                client.clone().start_polling_task();
                client
            }
            #[cfg(not(feature = "cloudrun"))]
            ExecutorType::CloudRun => {
                anyhow::bail!("CloudRun executor not enabled. Enable the 'cloudrun' feature flag.");
            }
        };

        let mut executors = self.executors.write().await;
        executors.insert(workflow.id, executor);
        Ok(())
    }

    async fn get_executor(&self, workflow_id: Uuid) -> Result<Arc<dyn Executor>> {
        let executors = self.executors.read().await;
        executors
            .get(&workflow_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Executor not found for workflow {}", workflow_id))
    }
}
