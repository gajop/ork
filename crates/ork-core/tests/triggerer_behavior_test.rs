use anyhow::Result;
use async_trait::async_trait;
use ork_core::database::{Database, NewTask};
use ork_core::job_tracker::{JobStatus, JobTracker};
use ork_core::triggerer::{Triggerer, TriggererConfig};
use ork_state::SqliteDatabase;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[derive(Clone)]
enum TrackerOutcome {
    Running,
    Completed,
    Failed(String),
    Error(String),
    SleepThen(Duration, JobStatus),
}

struct MockTracker {
    service_type: String,
    outcome: TrackerOutcome,
}

impl MockTracker {
    fn new(service_type: &str, outcome: TrackerOutcome) -> Self {
        Self {
            service_type: service_type.to_string(),
            outcome,
        }
    }
}

#[async_trait]
impl JobTracker for MockTracker {
    fn service_type(&self) -> &str {
        &self.service_type
    }

    async fn poll_job(
        &self,
        _job_id: &str,
        _job_data: &serde_json::Value,
    ) -> anyhow::Result<JobStatus> {
        match &self.outcome {
            TrackerOutcome::Running => Ok(JobStatus::Running),
            TrackerOutcome::Completed => Ok(JobStatus::Completed),
            TrackerOutcome::Failed(error) => Ok(JobStatus::Failed(error.clone())),
            TrackerOutcome::Error(error) => Err(anyhow::anyhow!("{}", error)),
            TrackerOutcome::SleepThen(delay, status) => {
                sleep(*delay).await;
                Ok(status.clone())
            }
        }
    }
}

async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
}

async fn create_deferred_job(
    db: &SqliteDatabase,
    workflow_name: &str,
    service_type: &str,
) -> Result<(Uuid, Uuid)> {
    let workflow = db
        .create_workflow(
            workflow_name,
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "test").await?;
    db.update_run_status(run.id, "running", None).await?;
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries: 0,
            timeout_seconds: Some(60),
        }],
    )
    .await?;
    let task = db
        .list_tasks(run.id)
        .await?
        .into_iter()
        .next()
        .expect("task exists");
    let job = db
        .create_deferred_job(
            task.id,
            service_type,
            &format!("job-{}", task.id),
            serde_json::json!({"status_url":"http://example.com"}),
        )
        .await?;
    Ok((task.id, job.id))
}

async fn wait_for_job_status(
    db: &SqliteDatabase,
    task_id: Uuid,
    wanted_status: &str,
) -> Result<()> {
    timeout(Duration::from_secs(2), async {
        loop {
            let jobs = db.get_deferred_jobs_for_task(task_id).await?;
            if jobs.iter().any(|j| j.status_str() == wanted_status) {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for deferred status {}", wanted_status))??;
    Ok(())
}

#[tokio::test]
async fn test_triggerer_sets_polling_for_running_jobs() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-running", "mock-running").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let mut triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(100),
        },
    );
    triggerer.register_tracker(Arc::new(MockTracker::new(
        "mock-running",
        TrackerOutcome::Running,
    )));
    triggerer.notify_new_job(task_id, "job-id", "mock-running");

    let handle = triggerer.start();
    wait_for_job_status(db.as_ref(), task_id, "polling").await?;

    let no_message = timeout(Duration::from_millis(100), completion_rx.recv()).await;
    assert!(
        no_message.is_err(),
        "running job should not emit completion"
    );
    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_triggerer_completes_job_and_notifies_scheduler() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-complete", "mock-complete").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let mut triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(100),
        },
    );
    triggerer.register_tracker(Arc::new(MockTracker::new(
        "mock-complete",
        TrackerOutcome::Completed,
    )));

    let handle = triggerer.start();
    wait_for_job_status(db.as_ref(), task_id, "completed").await?;

    let message = timeout(Duration::from_secs(1), completion_rx.recv())
        .await
        .expect("completion notification timeout")
        .expect("completion message");
    assert_eq!(message.task_id, task_id);
    assert!(message.success);
    assert!(message.error.is_none());

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_triggerer_marks_failed_and_notifies_on_tracker_failure() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-failed", "mock-failed").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let mut triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(100),
        },
    );
    triggerer.register_tracker(Arc::new(MockTracker::new(
        "mock-failed",
        TrackerOutcome::Failed("boom".to_string()),
    )));

    let handle = triggerer.start();
    wait_for_job_status(db.as_ref(), task_id, "failed").await?;

    let message = timeout(Duration::from_secs(1), completion_rx.recv())
        .await
        .expect("failure notification timeout")
        .expect("failure message");
    assert_eq!(message.task_id, task_id);
    assert!(!message.success);
    assert_eq!(message.error.as_deref(), Some("boom"));

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_triggerer_marks_failed_on_tracker_error() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-error", "mock-error").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let mut triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(100),
        },
    );
    triggerer.register_tracker(Arc::new(MockTracker::new(
        "mock-error",
        TrackerOutcome::Error("remote api unavailable".to_string()),
    )));

    let handle = triggerer.start();
    wait_for_job_status(db.as_ref(), task_id, "failed").await?;

    let message = timeout(Duration::from_secs(1), completion_rx.recv())
        .await
        .expect("error notification timeout")
        .expect("error message");
    assert_eq!(message.task_id, task_id);
    assert!(!message.success);
    assert!(
        message
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("remote api unavailable")
    );

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_triggerer_timeout_keeps_job_pending() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-timeout", "mock-timeout").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let mut triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(15),
        },
    );
    triggerer.register_tracker(Arc::new(MockTracker::new(
        "mock-timeout",
        TrackerOutcome::SleepThen(Duration::from_millis(200), JobStatus::Completed),
    )));

    let handle = triggerer.start();
    sleep(Duration::from_millis(80)).await;

    let jobs = db.get_deferred_jobs_for_task(task_id).await?;
    let job = jobs.first().expect("job exists");
    assert_eq!(job.status_str(), "pending");
    assert!(job.last_polled_at.is_some());
    let no_message = timeout(Duration::from_millis(100), completion_rx.recv()).await;
    assert!(no_message.is_err(), "timeout should not emit completion");

    handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_triggerer_missing_tracker_does_not_complete_job() -> Result<()> {
    let db = setup_db().await;
    let (task_id, _job_id) =
        create_deferred_job(db.as_ref(), "trigger-missing-tracker", "missing-service").await?;

    let (completion_tx, mut completion_rx) = mpsc::unbounded_channel();
    let triggerer = Triggerer::with_config(
        db.clone(),
        completion_tx,
        TriggererConfig {
            poll_interval: Duration::from_millis(5),
            max_jobs_per_cycle: 10,
            poll_timeout: Duration::from_millis(50),
        },
    );

    let handle = triggerer.start();
    sleep(Duration::from_millis(80)).await;

    let jobs = db.get_deferred_jobs_for_task(task_id).await?;
    let job = jobs.first().expect("job exists");
    assert_eq!(job.status_str(), "pending");
    assert!(job.last_polled_at.is_none());
    let no_message = timeout(Duration::from_millis(100), completion_rx.recv()).await;
    assert!(
        no_message.is_err(),
        "missing tracker should not emit completion"
    );

    handle.abort();
    Ok(())
}
