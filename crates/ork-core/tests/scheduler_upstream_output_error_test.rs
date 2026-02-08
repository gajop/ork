use anyhow::Result;
use async_trait::async_trait;
use ork_core::config::OrchestratorConfig;
use ork_core::database::{NewTask, RunRepository, TaskRepository, WorkflowRepository};
use ork_core::executor::{Executor, StatusUpdate};
use ork_core::executor_manager::ExecutorManager as ExecutorManagerTrait;
use ork_core::models::Workflow;
use ork_core::scheduler::Scheduler;
use ork_state::SqliteDatabase;
use sqlx::Row;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

struct AlwaysSuccessExecutor {
    tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<StatusUpdate>>>,
}

impl AlwaysSuccessExecutor {
    fn new() -> Self {
        Self {
            tx: Mutex::new(None),
        }
    }
}

#[async_trait]
impl Executor for AlwaysSuccessExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        _job_name: &str,
        _params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let guard = self.tx.lock().await;
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(StatusUpdate {
                task_id,
                status: "success".to_string(),
                log: None,
                output: Some(serde_json::json!({"ok": true})),
                error: None,
            });
        }
        Ok(format!("exec-{}", task_id))
    }

    async fn set_status_channel(&self, tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>) {
        let mut guard = self.tx.lock().await;
        *guard = Some(tx);
    }
}

struct AlwaysSuccessExecutorManager;

#[async_trait]
impl ExecutorManagerTrait for AlwaysSuccessExecutorManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        Ok(Arc::new(AlwaysSuccessExecutor::new()))
    }
}

async fn setup_db() -> Arc<SqliteDatabase> {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
    db.run_migrations().await.expect("migrations");
    db
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

async fn create_running_run_with_dependency_and_invalid_upstream(
    db: &SqliteDatabase,
    name: &str,
) -> (Uuid, Uuid) {
    let workflow = db
        .create_workflow(name, None, "job", "local", "local", "process", None, None)
        .await
        .expect("workflow");
    let run = db.create_run(workflow.id, "test").await.expect("run");
    db.batch_create_dag_tasks(
        run.id,
        &[
            NewTask {
                task_index: 0,
                task_name: "upstream".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo upstream"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
            NewTask {
                task_index: 1,
                task_name: "downstream".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["upstream".to_string()],
                params: serde_json::json!({"command":"echo downstream"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            },
        ],
    )
    .await
    .expect("create tasks");
    db.update_run_status(run.id, ork_core::models::RunStatus::Running, None)
        .await
        .expect("run running");

    let tasks = db.list_tasks(run.id).await.expect("list tasks");
    let upstream_task_id = tasks
        .iter()
        .find(|task| task.task_name == "upstream")
        .expect("upstream task")
        .id;
    let downstream_task_id = tasks
        .iter()
        .find(|task| task.task_name == "downstream")
        .expect("downstream task")
        .id;

    db.update_task_status(
        upstream_task_id,
        ork_core::models::TaskStatus::Success,
        Some("exec-upstream"),
        None,
    )
    .await
    .expect("mark upstream success");
    // Force invalid JSON so dependency output loading fails and exercises scheduler fallback path.
    sqlx::query("UPDATE tasks SET output = 'not-json' WHERE id = ?")
        .bind(upstream_task_id)
        .execute(db.pool())
        .await
        .expect("corrupt upstream output");

    (run.id, downstream_task_id)
}

#[tokio::test]
async fn test_scheduler_continues_when_upstream_output_decode_fails() -> Result<()> {
    let db = setup_db().await;
    let (run_id, downstream_task_id) = create_running_run_with_dependency_and_invalid_upstream(
        db.as_ref(),
        "invalid-upstream-output",
    )
    .await;

    let scheduler = Scheduler::new_with_config(
        db.clone(),
        Arc::new(AlwaysSuccessExecutorManager),
        test_config(),
    );
    let handle = tokio::spawn(async move {
        let _ = scheduler.run().await;
    });

    let wait = timeout(Duration::from_secs(3), async {
        loop {
            let run = db.get_run(run_id).await?;
            if run.status_str() == "success" {
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
        "timed out waiting for run completion with invalid upstream output"
    );
    let row = sqlx::query("SELECT status FROM tasks WHERE id = ?")
        .bind(downstream_task_id)
        .fetch_one(db.pool())
        .await?;
    let status: String = row.get("status");
    assert_eq!(status, "success");
    Ok(())
}
