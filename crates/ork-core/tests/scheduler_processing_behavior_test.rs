use anyhow::Result;
use async_trait::async_trait;
use ork_core::config::OrchestratorConfig;
use ork_core::database::{Database, NewTask};
use ork_core::executor::{Executor, StatusUpdate};
use ork_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use ork_core::models::Workflow;
use ork_core::scheduler::Scheduler;
use ork_state::SqliteDatabase;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum ExecMode {
    AlwaysSuccess,
    AlwaysFail,
    FailThenSuccess,
    DeferredSuccess,
    InvalidDeferred,
}

struct MockExecutor {
    mode: ExecMode,
    calls: AtomicUsize,
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<StatusUpdate>>>,
}

impl MockExecutor {
    fn new(mode: ExecMode) -> Self {
        Self {
            mode,
            calls: AtomicUsize::new(0),
            tx: Mutex::new(None),
        }
    }

    async fn send_success(&self, task_id: Uuid, output: serde_json::Value) -> anyhow::Result<()> {
        let guard = self.tx.lock().await;
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(StatusUpdate {
                task_id,
                status: "success".to_string(),
                log: None,
                output: Some(output),
                error: None,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl Executor for MockExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode {
            ExecMode::AlwaysFail => Err(anyhow::anyhow!("mock dispatch failure")),
            ExecMode::FailThenSuccess if call == 0 => Err(anyhow::anyhow!("first attempt fails")),
            ExecMode::DeferredSuccess => {
                self.send_success(
                    task_id,
                    serde_json::json!({
                        "deferred": [{
                            "service_type": "custom_http",
                            "job_id": format!("job-{}", task_id),
                            "url": "http://example.com/status",
                        }]
                    }),
                )
                .await?;
                Ok(format!("exec-{}", job_name))
            }
            ExecMode::InvalidDeferred => {
                self.send_success(
                    task_id,
                    serde_json::json!({
                        "deferred": [{
                            "service_type": "custom_http"
                        }]
                    }),
                )
                .await?;
                Ok(format!("exec-{}", job_name))
            }
            ExecMode::AlwaysSuccess | ExecMode::FailThenSuccess => {
                self.send_success(task_id, serde_json::json!({"ok": true}))
                    .await?;
                Ok(format!("exec-{}", job_name))
            }
        }
    }

    async fn set_status_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {
        let mut guard = self.tx.lock().await;
        *guard = Some(tx);
    }
}

struct MockExecutorManager {
    executor: Arc<MockExecutor>,
}

#[async_trait]
impl ExecutorManagerTrait for MockExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(self.executor.clone())
    }
}

async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
}

fn init_tracing() -> tracing::dispatcher::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_default(subscriber)
}

fn test_config() -> OrchestratorConfig {
    OrchestratorConfig {
        poll_interval_secs: 0.01,
        max_tasks_per_batch: 100,
        max_concurrent_dispatches: 10,
        max_concurrent_status_checks: 50,
        db_pool_size: 5,
        enable_triggerer: false,
    }
}

async fn create_running_run_with_task(
    db: &SqliteDatabase,
    name: &str,
    max_retries: i32,
    timeout_seconds: Option<i32>,
) -> (Uuid, Uuid) {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "process", None, None)
        .await
        .expect("workflow");
    let run = db.create_run(workflow.id, "test").await.expect("run");
    db.batch_create_dag_tasks(
        run.id,
        &[NewTask {
            task_index: 0,
            task_name: "task-a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo hi"}),
            max_retries,
            timeout_seconds,
        }],
    )
    .await
    .expect("create task");
    db.update_run_status(run.id, "running", None)
        .await
        .expect("run running");
    let task_id = db
        .list_tasks(run.id)
        .await
        .expect("list tasks")
        .into_iter()
        .next()
        .expect("task exists")
        .id;
    (run.id, task_id)
}

#[tokio::test]
async fn test_scheduler_non_dag_pending_run_creates_tasks() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let workflow = db
        .create_workflow(
            "legacy-pending-run",
            None,
            "job",
            "local",
            "local",
            "process",
            Some(serde_json::json!({"task_count": 2})),
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            if db.list_tasks(run.id).await?.len() >= 2 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(
        wait.is_ok(),
        "timed out waiting for scheduler to create tasks"
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_dag_pending_run_without_compiled_tasks_fails() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let workflow = db
        .create_workflow(
            "dag-missing-compiled-tasks",
            None,
            "dag",
            "local",
            "local",
            "dag",
            None,
            None,
        )
        .await?;
    let run = db.create_run(workflow.id, "manual").await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run_state = db.get_run(run.id).await?;
            if run_state.status_str() == "failed" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(
        wait.is_ok(),
        "timed out waiting for missing-compiled-tasks failure"
    );
    let run_state = db.get_run(run.id).await?;
    assert_eq!(run_state.status_str(), "failed");
    assert!(
        run_state
            .error
            .as_deref()
            .unwrap_or("")
            .contains("missing compiled workflow_tasks")
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_dispatch_failure_marks_run_failed() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "fail-run", 0, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysFail)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "failed" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for failed run status");
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    Ok(())
}

#[tokio::test]
async fn test_scheduler_retry_then_success() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, _task_id) =
        create_running_run_with_task(db.as_ref(), "retry-then-success", 1, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::FailThenSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(5), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "success" {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for retried run success");
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "success");
    assert!(task.attempts >= 1);
    Ok(())
}

#[tokio::test]
async fn test_scheduler_deferred_output_creates_deferred_job() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "deferred-output", 0, Some(10)).await;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::DeferredSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            if !db.get_deferred_jobs_for_task(task_id).await?.is_empty() {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for deferred job");
    let run = db.get_run(run_id).await?;
    assert_eq!(run.status_str(), "running");
    Ok(())
}

#[tokio::test]
async fn test_scheduler_invalid_deferred_payload_is_ignored() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "invalid-deferred-output", 0, Some(10)).await;

    let executor = Arc::new(MockExecutor::new(ExecMode::InvalidDeferred));
    let manager = Arc::new(MockExecutorManager {
        executor: executor.clone(),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            if executor.calls.load(Ordering::SeqCst) > 0 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    assert!(wait.is_ok(), "executor was never called");
    sleep(Duration::from_millis(200)).await;
    handle.abort();

    assert!(db.get_deferred_jobs_for_task(task_id).await?.is_empty());
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .find(|t| t.id == task_id)
        .expect("task exists");
    assert!(
        matches!(task.status_str(), "dispatched" | "running"),
        "expected task to remain non-terminal, got {}",
        task.status_str()
    );
    Ok(())
}

#[tokio::test]
async fn test_scheduler_enforces_timeout_for_stuck_task() -> Result<()> {
    let _trace = init_tracing();
    let db = setup_db().await;
    let (run_id, task_id) =
        create_running_run_with_task(db.as_ref(), "timeout-task", 0, Some(1)).await;

    db.update_task_status(task_id, "running", Some("exec"), None)
        .await?;
    sqlx::query("UPDATE tasks SET started_at = '2000-01-01T00:00:00Z' WHERE id = ?")
        .bind(task_id)
        .execute(db.pool())
        .await?;

    let manager = Arc::new(MockExecutorManager {
        executor: Arc::new(MockExecutor::new(ExecMode::AlwaysSuccess)),
    });
    let scheduler = Scheduler::new_with_config(db.clone(), manager, test_config());
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(2), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "failed" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        Ok::<(), anyhow::Error>(())
    })
    .await;
    handle.abort();

    assert!(wait.is_ok(), "timed out waiting for timeout enforcement");
    let task = db
        .list_tasks(run_id)
        .await?
        .into_iter()
        .next()
        .expect("task");
    assert_eq!(task.status_str(), "failed");
    assert!(
        task.error
            .as_deref()
            .unwrap_or_default()
            .contains("timeout")
    );
    Ok(())
}
