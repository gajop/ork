use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{Executor, StatusUpdate};

#[derive(Debug, Clone)]
enum ProcessStatus {
    Running,
    Success,
    Failed,
}

pub struct ProcessExecutor {
    working_dir: String,
    process_states: Arc<RwLock<HashMap<String, ProcessStatus>>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl ProcessExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
            process_states: Arc::new(RwLock::new(HashMap::new())),
            status_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn execute_process(
        &self,
        task_id: Uuid,
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
        let status_tx_clone = self.status_tx.clone();

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
            states.insert(exec_id_clone.clone(), status.clone());
            drop(states);

            // Send status update through channel if available
            let tx_guard = status_tx_clone.read().await;
            if let Some(tx) = tx_guard.as_ref() {
                let status_str = match status {
                    ProcessStatus::Running => "running",
                    ProcessStatus::Success => "success",
                    ProcessStatus::Failed => "failed",
                }.to_string();

                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: status_str,
                });
            }
        });

        Ok(execution_id)
    }

}

#[async_trait]
impl Executor for ProcessExecutor {
    async fn execute(&self, task_id: Uuid, job_name: &str, params: Option<serde_json::Value>) -> Result<String> {
        self.execute_process(task_id, job_name, params).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}
