use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct ProcessExecutor {
    working_dir: String,
}

impl ProcessExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
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

        // Clone execution_id for the async task
        let exec_id_clone = execution_id.clone();

        // For now, we'll spawn the process and let it run
        // In a real implementation, you might want to track the process better
        tokio::spawn(async move {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(&command_path);

            for (key, value) in env_vars.iter() {
                cmd.env(key, value);
            }

            match cmd.output() {
                Ok(output) => {
                    if output.status.success() {
                        info!("Process completed successfully: {}", exec_id_clone);
                        debug!("Output: {}", String::from_utf8_lossy(&output.stdout));
                    } else {
                        warn!("Process failed: {}", exec_id_clone);
                        warn!("Error: {}", String::from_utf8_lossy(&output.stderr));
                    }
                }
                Err(e) => {
                    warn!("Failed to execute process {}: {}", exec_id_clone, e);
                }
            }
        });

        Ok(execution_id)
    }

    pub async fn get_process_status(&self, _execution_id: &str) -> Result<String> {
        // For simplicity, we'll just return success immediately
        // In a real implementation, you would track process PIDs and check their status
        // For now, simulate quick completion
        debug!("Checking process status (simulated immediate success)");
        Ok("success".to_string())
    }
}
