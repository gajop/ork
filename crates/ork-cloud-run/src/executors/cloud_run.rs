use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::Executor;

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
    pub async fn new(project: String, region: String) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            project,
            region,
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
}

#[async_trait]
impl Executor for CloudRunClient {
    async fn execute(&self, job_name: &str, params: Option<serde_json::Value>) -> Result<String> {
        self.execute_job(job_name, params).await
    }

    async fn get_status(&self, execution_id: &str) -> Result<String> {
        self.get_execution_status(execution_id).await
    }
}
