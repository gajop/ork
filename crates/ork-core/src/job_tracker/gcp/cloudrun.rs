//! Cloud Run job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde_json::Value;

use crate::job_tracker::{JobStatus, JobTracker};

/// Cloud Run job tracker
///
/// Polls Cloud Run API to check job execution status.
pub struct CloudRunTracker {
    client: Option<google_cloud_run_v2::client::Executions>,
}

impl CloudRunTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let client = google_cloud_run_v2::client::Executions::builder()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Cloud Run client: {}", e))?;
        Ok(Self {
            client: Some(client),
        })
    }

    #[cfg(test)]
    pub(crate) fn without_client() -> Self {
        Self { client: None }
    }
}

#[async_trait]
impl JobTracker for CloudRunTracker {
    fn service_type(&self) -> &str {
        "cloudrun"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        let project = job_data
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project in job_data"))?;
        let region = job_data
            .get("region")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing region in job_data"))?;
        let job_name = job_data
            .get("job_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_name in job_data"))?;
        let execution_id = job_data
            .get("execution_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing execution_id in job_data"))?;

        tracing::debug!(
            "Polling Cloud Run job: project={}, region={}, job={}, execution={}",
            project,
            region,
            job_name,
            execution_id
        );

        // Build execution resource name
        let execution_name = format!(
            "projects/{}/locations/{}/jobs/{}/executions/{}",
            project, region, job_name, execution_id
        );

        // Call Cloud Run API to get execution status
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cloud Run client not initialized"))?;

        let execution = client
            .get_execution()
            .set_name(execution_name)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Cloud Run execution: {}", e))?;

        // Check execution conditions for completion
        use google_cloud_run_v2::model::condition::State;
        for condition in &execution.conditions {
            // Look for the "Ready" or "Completed" condition
            if condition.r#type == "Ready" || condition.r#type == "Completed" {
                match condition.state {
                    State::ConditionSucceeded => {
                        return Ok(JobStatus::Completed);
                    }
                    State::ConditionFailed => {
                        let error_msg = if condition.message.is_empty() {
                            "Execution failed".to_string()
                        } else {
                            condition.message.clone()
                        };
                        return Ok(JobStatus::Failed(format!(
                            "Cloud Run execution failed: {}",
                            error_msg
                        )));
                    }
                    State::ConditionPending | State::ConditionReconciling => {
                        // Still in progress
                    }
                    _ => {
                        // Unknown or unspecified state, continue checking
                    }
                }
            }
        }

        // If no definitive completion condition found, job is still running
        Ok(JobStatus::Running)
    }
}
