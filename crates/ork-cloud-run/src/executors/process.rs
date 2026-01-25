use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::Executor;

#[derive(Debug, Clone)]
enum ProcessStatus {
    Running,
    Success,
    Failed,
}

pub struct ProcessExecutor {
    working_dir: String,
    process_states: Arc<RwLock<HashMap<String, ProcessStatus>>>,
}

impl ProcessExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
            process_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn execute_process(
        &self,
        command: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let execution_id = Uuid::new_v4().to_string();

        info!(
            "Executing process: {} with execution_id: {}",
            command, execution_id
        );

        // Build environment variables from params
        let mut env_vars = HashMap::new();
        if let Some(params) = params {
            if let Some(obj) = params.as_object() {
                for (k, v) in obj.iter() {
                    env_vars.insert(k.clone(), v.to_string().trim_matches('"').to_string());
                }
            }
        }

        // Add execution ID to env
        env_vars.insert("EXECUTION_ID".to_string(), execution_id.clone());

        // Spawn the process in the background
        let command_path = format!("{}/{}", self.working_dir, command);

        debug!(
            "Spawning command: {} with env: {:?}",
            command_path, env_vars
        );

        // Mark as running
        {
            let mut states = self.process_states.write().await;
            states.insert(execution_id.clone(), ProcessStatus::Running);
        }

        // Clone for the async task
        let exec_id_clone = execution_id.clone();
        let states_clone = self.process_states.clone();

        tokio::spawn(async move {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(&command_path);

            for (key, value) in env_vars.iter() {
                cmd.env(key, value);
            }

            let status = match cmd.output().await {
                Ok(output) => {
                    if output.status.success() {
                        info!("Process completed successfully: {}", exec_id_clone);
                        debug!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        ProcessStatus::Success
                    } else {
                        warn!("Process failed: {}", exec_id_clone);
                        warn!("Error: {}", String::from_utf8_lossy(&output.stderr));
                        ProcessStatus::Failed
                    }
                }
                Err(e) => {
                    warn!("Failed to execute process {}: {}", exec_id_clone, e);
                    ProcessStatus::Failed
                }
            };

            // Update status
            let mut states = states_clone.write().await;
            states.insert(exec_id_clone, status);
        });

        Ok(execution_id)
    }

    pub async fn get_process_status(&self, execution_id: &str) -> Result<String> {
        let states = self.process_states.read().await;

        match states.get(execution_id) {
            Some(ProcessStatus::Running) => {
                debug!("Process {} is still running", execution_id);
                Ok("running".to_string())
            }
            Some(ProcessStatus::Success) => {
                debug!("Process {} completed successfully", execution_id);
                Ok("success".to_string())
            }
            Some(ProcessStatus::Failed) => {
                debug!("Process {} failed", execution_id);
                Ok("failed".to_string())
            }
            None => {
                warn!("Unknown process execution_id: {}", execution_id);
                Ok("unknown".to_string())
            }
        }
    }
}

#[async_trait]
impl Executor for ProcessExecutor {
    async fn execute(&self, job_name: &str, params: Option<serde_json::Value>) -> Result<String> {
        self.execute_process(job_name, params).await
    }

    async fn get_status(&self, execution_id: &str) -> Result<String> {
        self.get_process_status(execution_id).await
    }
}
