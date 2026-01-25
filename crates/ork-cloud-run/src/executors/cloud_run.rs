use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::{Executor, StatusUpdate};

#[derive(Debug, Serialize)]
pub struct CloudRunJobRequest {
    pub overrides: Overrides,
}

#[derive(Debug, Serialize)]
pub struct Overrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_overrides: Option<Vec<ContainerOverride>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ContainerOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Vec<EnvVar>>,
}

#[derive(Debug, Serialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudRunExecution {
    pub name: String,
}

#[derive(Debug, Clone)]
struct ExecutionState {
    task_id: Uuid,
    execution_name: String,
    last_status: String,
}

pub struct CloudRunClient {
    client: Client,
    project: String,
    region: String,
    active_executions: Arc<RwLock<HashMap<String, ExecutionState>>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
    poll_interval_secs: u64,
}

impl CloudRunClient {
    pub async fn new(project: String, region: String) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            project,
            region,
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            status_tx: Arc::new(RwLock::new(None)),
            poll_interval_secs: 5,
        })
    }

    async fn get_access_token(&self) -> Result<String> {
        let provider = gcp_auth::provider().await
            .context("Failed to get GCP auth provider. Set GOOGLE_APPLICATION_CREDENTIALS or use workload identity")?;
        let scopes = &["https://www.googleapis.com/auth/cloud-platform"];
        let token = provider.token(scopes).await
            .context("Failed to get access token")?;
        debug!("Got access token from GCP auth");
        Ok(token.as_str().to_string())
    }

    pub async fn execute_job(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let token = self.get_access_token().await?;

        let url = format!(
            "https://{}-run.googleapis.com/v1/namespaces/{}/jobs/{}:run",
            self.region, self.project, job_name
        );

        // Build environment variables from params
        let env_vars: Vec<EnvVar> = if let Some(ref params) = params {
            params
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| EnvVar {
                            name: k.clone(),
                            value: v.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let request = CloudRunJobRequest {
            overrides: Overrides {
                container_overrides: if !env_vars.is_empty() {
                    Some(vec![ContainerOverride {
                        env: Some(env_vars),
                    }])
                } else {
                    None
                },
                task_count: Some(1),
            },
        };

        debug!(
            "Executing Cloud Run job: {} with params: {:?}",
            job_name, params
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&request)
            .send()
            .await
            .context("Failed to send Cloud Run job request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Cloud Run job execution failed with status {}: {}",
                status,
                error_text
            );
        }

        let execution: CloudRunExecution = response
            .json()
            .await
            .context("Failed to parse Cloud Run execution response")?;

        info!(
            "Successfully started Cloud Run job execution: {}",
            execution.name
        );

        // Track this execution
        {
            let mut executions = self.active_executions.write().await;
            executions.insert(
                execution.name.clone(),
                ExecutionState {
                    task_id,
                    execution_name: execution.name.clone(),
                    last_status: "running".to_string(),
                },
            );
        }

        Ok(execution.name)
    }

    pub async fn get_execution_status(&self, execution_name: &str) -> Result<String> {
        let token = self.get_access_token().await?;

        let url = format!(
            "https://{}-run.googleapis.com/v1/{}",
            self.region, execution_name
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to get Cloud Run execution status")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Failed to get execution status {}: {}", status, error_text);
        }

        let execution: serde_json::Value = response.json().await?;

        // Extract status from the response
        let status = execution
            .get("status")
            .and_then(|s| s.get("conditions"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cond| cond.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        Ok(status.to_lowercase())
    }

    async fn poll_active_executions(&self) {
        let executions_to_check = {
            let executions = self.active_executions.read().await;
            executions.values().cloned().collect::<Vec<_>>()
        };

        if executions_to_check.is_empty() {
            return;
        }

        for exec_state in executions_to_check {
            match self.get_execution_status(&exec_state.execution_name).await {
                Ok(status) => {
                    let normalized_status = match status.as_str() {
                        "completed" | "succeeded" => "success",
                        "failed" | "error" => "failed",
                        _ => "running",
                    };

                    if normalized_status != exec_state.last_status {
                        let mut executions = self.active_executions.write().await;

                        if let Some(state) = executions.get_mut(&exec_state.execution_name) {
                            state.last_status = normalized_status.to_string();
                        }

                        let tx_guard = self.status_tx.read().await;
                        if let Some(tx) = tx_guard.as_ref() {
                            let _ = tx.send(StatusUpdate {
                                task_id: exec_state.task_id,
                                status: normalized_status.to_string(),
                            });
                        }

                        if normalized_status == "success" || normalized_status == "failed" {
                            let mut executions = self.active_executions.write().await;
                            executions.remove(&exec_state.execution_name);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get status for {}: {}", exec_state.execution_name, e);
                }
            }
        }
    }

    pub fn start_polling_task(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(self.poll_interval_secs)).await;
                self.poll_active_executions().await;
            }
        });
    }
}

#[async_trait]
impl Executor for CloudRunClient {
    async fn execute(&self, task_id: Uuid, job_name: &str, params: Option<serde_json::Value>) -> Result<String> {
        self.execute_job(task_id, job_name, params).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}
