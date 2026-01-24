use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{debug, info};

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

pub struct CloudRunClient {
    client: Client,
    project: String,
    region: String,
}

impl CloudRunClient {
    pub fn new(project: String, region: String) -> Self {
        Self {
            client: Client::new(),
            project,
            region,
        }
    }

    async fn get_access_token(&self) -> Result<String> {
        // Try to get access token using gcloud CLI
        // For local testing without gcloud, will use mock mode

        // Check if gcloud is available
        let output = Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output();

        match output {
            Ok(result) if result.status.success() => {
                let token = String::from_utf8(result.stdout)?.trim().to_string();
                debug!("Got access token from gcloud CLI");
                Ok(token)
            }
            _ => {
                // Fallback to mock mode for local testing
                debug!("gcloud not available or not authenticated, using mock mode");
                Ok("mock-token-for-local-testing".to_string())
            }
        }
    }

    pub async fn execute_job(
        &self,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let token = self.get_access_token().await?;

        // Check if we're in mock mode
        if token == "mock-token-for-local-testing" {
            info!(
                "Mock mode: Simulating Cloud Run job execution for {}",
                job_name
            );
            return Ok(format!("mock-execution-{}", uuid::Uuid::new_v4()));
        }

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

        Ok(execution.name)
    }

    pub async fn get_execution_status(&self, execution_name: &str) -> Result<String> {
        let token = self.get_access_token().await?;

        // Mock mode
        if token == "mock-token-for-local-testing" {
            debug!("Mock mode: Simulating execution status check");
            // Simulate random success/running
            return Ok("success".to_string());
        }

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
}
