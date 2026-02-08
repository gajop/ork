//! Dataproc job tracker for Google Cloud Platform

use async_trait::async_trait;
use serde_json::Value;

use crate::job_tracker::{JobStatus, JobTracker};

/// Dataproc job tracker
///
/// Polls Dataproc API to check Spark/Hadoop job status.
pub struct DataprocTracker {
    client: Option<google_cloud_dataproc_v1::client::JobController>,
}

impl DataprocTracker {
    pub async fn new() -> anyhow::Result<Self> {
        let client = google_cloud_dataproc_v1::client::JobController::builder()
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create Dataproc client: {}", e))?;
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
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dataproc client not initialized"))?;

        let job = client
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
                    if !status.details.is_empty() && status.details.to_lowercase().contains("error")
                    {
                        return Ok(JobStatus::Failed(format!(
                            "Dataproc job completed with errors: {}",
                            status.details
                        )));
                    }
                    Ok(JobStatus::Completed)
                }
                State::Error => {
                    let error_msg = if status.details.is_empty() {
                        "Job error".to_string()
                    } else {
                        status.details
                    };
                    Ok(JobStatus::Failed(format!(
                        "Dataproc job error: {}",
                        error_msg
                    )))
                }
                State::Cancelled => Ok(JobStatus::Failed("Job was cancelled".to_string())),
                State::Pending
                | State::SetupDone
                | State::Running
                | State::CancelPending
                | State::CancelStarted => Ok(JobStatus::Running),
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
