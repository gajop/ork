//! Job tracking for deferrables
//!
//! This module defines the JobTracker trait and implementations for polling
//! external APIs to track long-running jobs (BigQuery, Cloud Run, Dataproc, etc.)

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Status of an external job being tracked
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is still running
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed(String),
}

/// Trait for tracking external job status
///
/// Implementations poll external APIs to check job status.
/// Used by the Triggerer component.
#[async_trait]
pub trait JobTracker: Send + Sync {
    /// Get the service type this tracker handles
    fn service_type(&self) -> &str;

    /// Poll the external API to check job status
    ///
    /// # Arguments
    /// * `job_id` - External job identifier
    /// * `job_data` - Full deferrable JSON data (project, location, etc.)
    ///
    /// # Returns
    /// Current job status
    async fn poll_job(&self, job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus>;
}

/// BigQuery job tracker
///
/// Polls BigQuery API to check query job status.
pub struct BigQueryTracker {
    client: google_cloud_bigquery::client::Client,
}

impl BigQueryTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let (config, _project) = google_cloud_bigquery::client::ClientConfig::new_with_auth()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create BigQuery client config: {}", e))?;
        let client = google_cloud_bigquery::client::Client::new(config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create BigQuery client: {}", e))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl JobTracker for BigQueryTracker {
    fn service_type(&self) -> &str {
        "bigquery"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        // Extract BigQuery job info
        let project = job_data
            .get("project")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing project in job_data"))?;
        let location = job_data
            .get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing location in job_data"))?;
        let bq_job_id = job_data
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_id in job_data"))?;

        tracing::debug!(
            "Polling BigQuery job: project={}, location={}, job_id={}",
            project,
            location,
            bq_job_id
        );

        // Call BigQuery API to get job status
        let get_request = google_cloud_bigquery::http::job::get::GetJobRequest {
            location: Some(location.to_string()),
            ..Default::default()
        };

        let job = self.client
            .job()
            .get(project, bq_job_id, &get_request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get BigQuery job: {}", e))?;

        // Check job status
        use google_cloud_bigquery::http::job::JobState;
        match job.status.state {
            JobState::Done => {
                // Check for errors
                if let Some(error_result) = job.status.error_result {
                    let error_msg = error_result.message.unwrap_or_else(|| "Unknown error".to_string());
                    return Ok(JobStatus::Failed(format!("BigQuery job failed: {}", error_msg)));
                }
                Ok(JobStatus::Completed)
            }
            JobState::Pending | JobState::Running => Ok(JobStatus::Running),
        }
    }
}

/// Cloud Run job tracker
///
/// Polls Cloud Run API to check job execution status.
pub struct CloudRunTracker {
    client: google_cloud_run_v2::client::Executions,
}

impl CloudRunTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let client = google_cloud_run_v2::client::Executions::builder()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Cloud Run client: {}", e))?;
        Ok(Self { client })
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
        let execution = self.client
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
                        return Ok(JobStatus::Failed(format!("Cloud Run execution failed: {}", error_msg)));
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

/// Dataproc job tracker
///
/// Polls Dataproc API to check Spark/Hadoop job status.
pub struct DataprocTracker {
    client: google_cloud_dataproc_v1::client::JobController,
}

impl DataprocTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let client = google_cloud_dataproc_v1::client::JobController::builder()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Dataproc client: {}", e))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl JobTracker for DataprocTracker {
    fn service_type(&self) -> &str {
        "dataproc"
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
        let _cluster_name = job_data
            .get("cluster_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing cluster_name in job_data"))?;
        let dp_job_id = job_data
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_id in job_data"))?;

        tracing::debug!(
            "Polling Dataproc job: project={}, region={}, job_id={}",
            project,
            region,
            dp_job_id
        );

        // Call Dataproc API to get job status
        let job = self.client
            .get_job()
            .set_project_id(project)
            .set_region(region)
            .set_job_id(dp_job_id)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Dataproc job: {}", e))?;

        // Check job status state
        if let Some(status) = job.status {
            use google_cloud_dataproc_v1::model::job_status::State;
            match status.state {
                State::Done => {
                    // Job completed - check for errors in details
                    if !status.details.is_empty() && status.details.to_lowercase().contains("error") {
                        return Ok(JobStatus::Failed(format!("Dataproc job completed with errors: {}", status.details)));
                    }
                    Ok(JobStatus::Completed)
                }
                State::Error => {
                    let error_msg = if status.details.is_empty() {
                        "Job error".to_string()
                    } else {
                        status.details
                    };
                    Ok(JobStatus::Failed(format!("Dataproc job error: {}", error_msg)))
                }
                State::Cancelled => {
                    Ok(JobStatus::Failed("Job was cancelled".to_string()))
                }
                State::Pending | State::SetupDone | State::Running |
                State::CancelPending | State::CancelStarted => {
                    Ok(JobStatus::Running)
                }
                _ => {
                    // Unknown state, assume running
                    Ok(JobStatus::Running)
                }
            }
        } else {
            // No status yet, assume running
            Ok(JobStatus::Running)
        }
    }
}

/// Custom HTTP job tracker
///
/// Polls a custom HTTP endpoint to check job status.
pub struct CustomHttpTracker {
    client: reqwest::Client,
}

impl CustomHttpTracker {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for CustomHttpTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl JobTracker for CustomHttpTracker {
    fn service_type(&self) -> &str {
        "custom_http"
    }

    async fn poll_job(&self, _job_id: &str, job_data: &Value) -> anyhow::Result<JobStatus> {
        let url = job_data
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing url in job_data"))?;

        let method = job_data
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let headers = job_data
            .get("headers")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        let success_status_codes = job_data
            .get("success_status_codes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|n| n as u16))
                    .collect::<Vec<u16>>()
            })
            .unwrap_or_else(|| vec![200]);

        let completion_field = job_data
            .get("completion_field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing completion_field in job_data"))?;

        let completion_value = job_data
            .get("completion_value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing completion_value in job_data"))?;

        let failure_value = job_data
            .get("failure_value")
            .and_then(|v| v.as_str());

        // Build HTTP request
        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(&key, &value);
        }

        // Send request
        let response = request.send().await?;
        let status_code = response.status().as_u16();

        // Check if status code is successful
        if !success_status_codes.contains(&status_code) {
            return Ok(JobStatus::Failed(format!(
                "HTTP request failed with status {}",
                status_code
            )));
        }

        // Parse response JSON
        let body: Value = response.json().await?;

        // Check completion field
        let field_value = body
            .get(completion_field)
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Response missing completion field '{}' or not a string",
                    completion_field
                )
            })?;

        // Check if job is completed
        if field_value == completion_value {
            return Ok(JobStatus::Completed);
        }

        // Check if job failed
        if let Some(fail_val) = failure_value {
            if field_value == fail_val {
                return Ok(JobStatus::Failed(format!(
                    "Job failed with status: {}",
                    field_value
                )));
            }
        }

        // Job is still running
        Ok(JobStatus::Running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bigquery_tracker_service_type() {
        let tracker = BigQueryTracker::new();
        assert_eq!(tracker.service_type(), "bigquery");
    }

    #[test]
    fn test_cloudrun_tracker_service_type() {
        let tracker = CloudRunTracker::new();
        assert_eq!(tracker.service_type(), "cloudrun");
    }

    #[test]
    fn test_dataproc_tracker_service_type() {
        let tracker = DataprocTracker::new();
        assert_eq!(tracker.service_type(), "dataproc");
    }

    #[test]
    fn test_custom_http_tracker_service_type() {
        let tracker = CustomHttpTracker::new();
        assert_eq!(tracker.service_type(), "custom_http");
    }

    #[tokio::test]
    async fn test_bigquery_poll_validates_job_data() {
        let tracker = BigQueryTracker::new();
        let invalid_data = json!({});
        let result = tracker.poll_job("test-job", &invalid_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing project"));
    }

    #[tokio::test]
    async fn test_cloudrun_poll_validates_job_data() {
        let tracker = CloudRunTracker::new();
        let invalid_data = json!({});
        let result = tracker.poll_job("test-job", &invalid_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing project"));
    }

    #[tokio::test]
    async fn test_dataproc_poll_validates_job_data() {
        let tracker = DataprocTracker::new().await.unwrap();
        let invalid_data = json!({});
        let result = tracker.poll_job("test-job", &invalid_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing project"));
    }
}
