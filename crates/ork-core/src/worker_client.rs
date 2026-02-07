//! Worker client for calling worker HTTP endpoints
//!
//! The scheduler uses this client to:
//! - Compile workflows via POST /compile
//! - Execute tasks via POST /execute

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Worker client for HTTP communication with worker containers
pub struct WorkerClient {
    client: reqwest::Client,
    worker_url: String,
}

#[derive(Debug, Serialize)]
pub struct CompileRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompileResponse {
    pub workflow: serde_json::Value,
    pub tasks: Vec<TaskDefinition>,
}

#[derive(Debug, Deserialize)]
pub struct TaskDefinition {
    pub name: String,
    pub executor: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteRequest {
    pub task_id: Uuid,
    pub task_name: String,
    pub executor_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status")]
pub enum ExecuteResponse {
    #[serde(rename = "success")]
    Success {
        output: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        deferred: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "failed")]
    Failed {
        error: String,
    },
}

impl WorkerClient {
    /// Create a new worker client
    ///
    /// # Arguments
    /// * `worker_url` - Base URL of the worker server (e.g., "http://localhost:8081")
    pub fn new(worker_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for task execution
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            worker_url,
        }
    }

    /// Compile a workflow by calling POST /compile
    pub async fn compile(&self, workflow_path: Option<String>) -> Result<CompileResponse> {
        let url = format!("{}/compile", self.worker_url);
        let req = CompileRequest { workflow_path };

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Compile request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let compile_response: CompileResponse = response.json().await?;
        Ok(compile_response)
    }

    /// Execute a task by calling POST /execute
    pub async fn execute(&self, req: ExecuteRequest) -> Result<ExecuteResponse> {
        let url = format!("{}/execute", self.worker_url);

        tracing::debug!("Calling worker execute: task={}, executor={}", req.task_name, req.executor_type);

        let response = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Execute request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let execute_response: ExecuteResponse = response.json().await?;
        Ok(execute_response)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        let url = format!("{}/health", self.worker_url);

        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Health check failed with status {}",
                response.status()
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_client_creation() {
        let client = WorkerClient::new("http://localhost:8081".to_string());
        assert_eq!(client.worker_url, "http://localhost:8081");
    }
}
