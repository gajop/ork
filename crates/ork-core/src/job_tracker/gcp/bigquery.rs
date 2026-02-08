//! BigQuery job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde_json::Value;

use crate::job_tracker::{JobStatus, JobTracker};

/// BigQuery job tracker
///
/// Polls BigQuery API to check query job status.
pub struct BigQueryTracker {
    client: Option<google_cloud_bigquery::client::Client>,
}

impl BigQueryTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let (config, _project) = google_cloud_bigquery::client::ClientConfig::new_with_auth()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create BigQuery client config: {}", e))?;
        let client = google_cloud_bigquery::client::Client::new(config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create BigQuery client: {}", e))?;
        Ok(Self {
            client: Some(client),
        })
    }

    #[cfg(test)]
    pub(crate) fn without_client() -> Self {
        Self { client: None }
    }
}

pub(crate) fn map_bigquery_status(
    status: &google_cloud_bigquery::http::job::JobStatus,
) -> JobStatus {
    use google_cloud_bigquery::http::job::JobState;
    match status.state {
        JobState::Done => {
            if let Some(error_result) = status.error_result.as_ref() {
                let error_msg = error_result
                    .message
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string());
                JobStatus::Failed(format!("BigQuery job failed: {}", error_msg))
            } else {
                JobStatus::Completed
            }
        }
        JobState::Pending | JobState::Running => JobStatus::Running,
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
        };

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("BigQuery client not initialized"))?;

        let job = client
            .job()
            .get(project, bq_job_id, &get_request)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get BigQuery job: {}", e))?;

        Ok(map_bigquery_status(&job.status))
    }
}
