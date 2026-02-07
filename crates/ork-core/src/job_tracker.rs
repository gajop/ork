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
    // In a real implementation, this would hold BigQuery client
    // For now, we'll use a placeholder
    _placeholder: (),
}

impl BigQueryTracker {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for BigQueryTracker {
    fn default() -> Self {
        Self::new()
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

        // TODO: Implement actual BigQuery API polling
        // For now, return a placeholder
        tracing::debug!(
            "Polling BigQuery job: project={}, location={}, job_id={}",
            project,
            location,
            bq_job_id
        );

        // Placeholder: always return running
        // In real implementation:
        // 1. Use google-cloud-bigquery crate
        // 2. Call jobs().get(project, job_id) API
        // 3. Check job.status.state (PENDING, RUNNING, DONE)
        // 4. Check job.status.error_result for failures
        Ok(JobStatus::Running)
    }
}

/// Cloud Run job tracker
///
/// Polls Cloud Run API to check job execution status.
pub struct CloudRunTracker {
    _placeholder: (),
}

impl CloudRunTracker {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for CloudRunTracker {
    fn default() -> Self {
        Self::new()
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

        // TODO: Implement actual Cloud Run API polling
        tracing::debug!(
            "Polling Cloud Run job: project={}, region={}, job={}, execution={}",
            project,
            region,
            job_name,
            execution_id
        );

        // Placeholder: always return running
        // In real implementation:
        // 1. Use google-cloud-run crate or REST API
        // 2. Call executions().get() API
        // 3. Check execution.status.conditions for completion
        Ok(JobStatus::Running)
    }
}

/// Dataproc job tracker
///
/// Polls Dataproc API to check Spark/Hadoop job status.
pub struct DataprocTracker {
    _placeholder: (),
}

impl DataprocTracker {
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for DataprocTracker {
    fn default() -> Self {
        Self::new()
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
        let cluster_name = job_data
            .get("cluster_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing cluster_name in job_data"))?;
        let dp_job_id = job_data
            .get("job_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing job_id in job_data"))?;

        // TODO: Implement actual Dataproc API polling
        tracing::debug!(
            "Polling Dataproc job: project={}, region={}, cluster={}, job_id={}",
            project,
            region,
            cluster_name,
            dp_job_id
        );

        // Placeholder: always return running
        // In real implementation:
        // 1. Use google-cloud-dataproc crate
        // 2. Call jobs().get() API
        // 3. Check job.status.state (PENDING, RUNNING, DONE, ERROR, CANCELLED)
        Ok(JobStatus::Running)
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
        let tracker = DataprocTracker::new();
        let invalid_data = json!({});
        let result = tracker.poll_job("test-job", &invalid_data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Missing project"));
    }
}
