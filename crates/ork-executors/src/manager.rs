use anyhow::Result;
use async_trait::async_trait;
#[cfg(feature = "cloudrun")]
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "cloudrun")]
use tokio::sync::RwLock;

use ork_core::executor::Executor;
use ork_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use ork_core::models::{ExecutorType, Workflow};

#[cfg(feature = "cloudrun")]
use crate::cloud_run::CloudRunClient;
#[cfg(feature = "process")]
use crate::process::ProcessExecutor;
#[cfg(feature = "library")]
use crate::library::LibraryExecutor;

pub struct ExecutorManager {
    #[cfg(feature = "process")]
    process_executor: Arc<ProcessExecutor>,
    #[cfg(feature = "library")]
    library_executor: Arc<LibraryExecutor>,
    #[cfg(feature = "cloudrun")]
    cloudrun_clients: Arc<RwLock<HashMap<(String, String), Arc<CloudRunClient>>>>,
}

impl ExecutorManager {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "process")]
            process_executor: Arc::new(ProcessExecutor::new(None)), // Use current directory
            #[cfg(feature = "library")]
            library_executor: Arc::new(LibraryExecutor::new()),
            #[cfg(feature = "cloudrun")]
            cloudrun_clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ExecutorManagerTrait for ExecutorManager {
    async fn get_executor(
        &self,
        executor_type: &str,
        workflow: &Workflow,
    ) -> Result<Arc<dyn Executor>> {
        #[cfg(not(feature = "cloudrun"))]
        let _ = workflow;
        let executor_type = ExecutorType::from_str(executor_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown executor type: {}", executor_type))?;

        let executor: Arc<dyn Executor> = match executor_type {
            #[cfg(feature = "process")]
            ExecutorType::Process => self.process_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Process => {
                anyhow::bail!("Process executor not enabled. Enable the 'process' feature flag.");
            }
            #[cfg(feature = "process")]
            ExecutorType::Python => self.process_executor.clone(),
            #[cfg(not(feature = "process"))]
            ExecutorType::Python => {
                anyhow::bail!("Python executor not enabled. Enable the 'process' feature flag.");
            }
            #[cfg(feature = "library")]
            ExecutorType::Library => self.library_executor.clone(),
            #[cfg(not(feature = "library"))]
            ExecutorType::Library => {
                anyhow::bail!("Library executor not enabled. Enable the 'library' feature flag.");
            }
            #[cfg(feature = "cloudrun")]
            ExecutorType::CloudRun => {
                let key = (workflow.project.clone(), workflow.region.clone());
                let existing = {
                    let clients = self.cloudrun_clients.read().await;
                    clients.get(&key).cloned()
                };
                if let Some(client) = existing {
                    client
                } else {
                    let client = Arc::new(CloudRunClient::new(key.0.clone(), key.1.clone()).await?);
                    client.clone().start_polling_task();
                    let mut clients = self.cloudrun_clients.write().await;
                    clients.insert(key, client.clone());
                    client
                }
            }
            #[cfg(not(feature = "cloudrun"))]
            ExecutorType::CloudRun => {
                anyhow::bail!("CloudRun executor not enabled. Enable the 'cloudrun' feature flag.");
            }
        };

        Ok(executor)
    }
}
