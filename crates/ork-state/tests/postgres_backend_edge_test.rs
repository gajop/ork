#![cfg(feature = "postgres")]

use anyhow::Result;
use chrono::{Duration, Utc};
use ork_core::database::{Database, NewTask, NewWorkflowTask};
use ork_state::PostgresDatabase;
use url::Url;
use uuid::Uuid;

struct PostgresTestContext {
    admin: PostgresDatabase,
    db_name: String,
    db: PostgresDatabase,
}

impl PostgresTestContext {
    async fn setup() -> Result<Option<Self>> {
        let base_url = postgres_test_url();
        let admin_url = with_database(&base_url, "postgres")?;
        let admin = match PostgresDatabase::new(&admin_url).await {
            Ok(db) => db,
            Err(err) => {
                eprintln!(
                    "Skipping Postgres coverage test: failed connecting to {} ({})",
                    admin_url, err
                );
                return Ok(None);
            }
        };

        let db_name = format!("cov_{}", Uuid::new_v4().simple());
        let create_database_sql = format!(r#"CREATE DATABASE "{}""#, db_name);
        sqlx::query(&create_database_sql)
            .execute(admin.pool())
            .await?;

        let db_url = with_database(&base_url, &db_name)?;
        let db = PostgresDatabase::new(&db_url).await?;
        Database::run_migrations(&db).await?;

        Ok(Some(Self { admin, db_name, db }))
    }

    async fn cleanup(self) -> Result<()> {
        self.db.pool().close().await;
        sqlx::query(
            "SELECT pg_terminate_backend(pid)
             FROM pg_stat_activity
             WHERE datname = $1 AND pid <> pg_backend_pid()",
        )
        .bind(&self.db_name)
        .execute(self.admin.pool())
        .await?;
        let drop_database_sql = format!(r#"DROP DATABASE IF EXISTS "{}""#, self.db_name);
        sqlx::query(&drop_database_sql)
            .execute(self.admin.pool())
            .await?;
        Ok(())
    }
}

fn postgres_test_url() -> String {
    std::env::var("ORK_POSTGRES_TEST_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/orchestrator".to_string())
}

fn with_database(base_url: &str, database: &str) -> Result<String> {
    let mut url = Url::parse(base_url)?;
    url.set_path(&format!("/{database}"));
    Ok(url.to_string())
}

fn task_id(tasks: &[ork_core::models::Task], name: &str) -> Uuid {
    tasks
        .iter()
        .find(|t| t.task_name == name)
        .unwrap_or_else(|| panic!("missing task: {name}"))
        .id
}
#[tokio::test]
async fn test_postgres_branch_and_edge_paths() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let wf = Database::create_workflow(
        db,
        "pg-edges",
        Some("edge cases"),
        "job",
        "region",
        "project",
        "dag",
        None,
        None,
    )
    .await?;
    let workflow_id = wf.id;

    let by_ids_empty = Database::get_workflows_by_ids(db, &[]).await?;
    assert!(by_ids_empty.is_empty());
    let due_initial = Database::get_due_scheduled_workflows(db).await?;
    assert!(!due_initial.iter().any(|w| w.id == workflow_id));

    Database::create_workflow_tasks(
        db,
        workflow_id,
        &[NewWorkflowTask {
            task_index: 0,
            task_name: "extract".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: serde_json::json!({"command":"echo extract"}),
            signature: None,
        }],
    )
    .await?;
    Database::create_workflow_tasks(
        db,
        workflow_id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                signature: Some(serde_json::json!({"output":"json"})),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                signature: None,
            },
        ],
    )
    .await?;
    let workflow_tasks = Database::list_workflow_tasks(db, workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    let run = Database::create_run(db, workflow_id, "tests").await?;
    let run_id = run.id;
    let pending_runs = Database::get_pending_runs(db).await?;
    assert!(pending_runs.iter().any(|r| r.id == run_id));
    let runs_all = Database::list_runs(db, None).await?;
    assert!(runs_all.iter().any(|r| r.id == run_id));

    Database::batch_create_dag_tasks(
        db,
        run_id,
        &[
            NewTask {
                task_index: 0,
                task_name: "extract".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo extract"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
            NewTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                max_retries: 3,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    Database::batch_update_task_status(db, &[]).await?;
    let no_failed =
        Database::mark_tasks_failed_by_dependency(db, run_id, &[], "should not apply").await?;
    assert!(no_failed.is_empty());

    let empty_outputs = Database::get_task_outputs(db, run_id, &[]).await?;
    assert!(empty_outputs.is_empty());

    let tasks = Database::list_tasks(db, run_id).await?;
    let extract_id = task_id(&tasks, "extract");
    let load_id = task_id(&tasks, "load");

    let empty_retry = Database::get_task_retry_meta(db, &[]).await?;
    assert!(empty_retry.is_empty());

    let retry_at = Utc::now() + Duration::seconds(30);
    Database::reset_task_for_retry(db, extract_id, Some("retry"), Some(retry_at)).await?;
    let mut tasks = Database::list_tasks(db, run_id).await?;
    let extract_after_reset = tasks
        .iter()
        .find(|t| t.id == extract_id)
        .expect("extract after reset");
    assert!(extract_after_reset.retry_at.is_some());

    Database::update_task_status(db, extract_id, "paused", None, Some("paused")).await?;
    tasks = Database::list_tasks(db, run_id).await?;
    let extract_paused = tasks.iter().find(|t| t.id == extract_id).expect("paused");
    assert_eq!(extract_paused.status_str(), "paused");
    assert!(extract_paused.retry_at.is_some());

    Database::update_task_status(db, extract_id, "running", Some("exec-edge"), None).await?;
    tasks = Database::list_tasks(db, run_id).await?;
    let extract_running = tasks.iter().find(|t| t.id == extract_id).expect("running");
    assert_eq!(extract_running.status_str(), "running");
    assert!(extract_running.retry_at.is_none());
    assert!(extract_running.started_at.is_some());

    Database::update_run_status(db, run_id, "running", None).await?;
    let run_running = Database::get_run(db, run_id).await?;
    assert_eq!(run_running.status_str(), "running");
    assert!(run_running.started_at.is_some());
    assert!(run_running.finished_at.is_none());

    let pending_with_unsatisfied = Database::get_pending_tasks_with_workflow(db, 10).await?;
    assert!(
        pending_with_unsatisfied
            .iter()
            .all(|t| t.task_name != "load")
    );

    Database::update_task_status(db, extract_id, "success", Some("exec-edge"), None).await?;
    let pending_with_satisfied = Database::get_pending_tasks_with_workflow(db, 10).await?;
    assert!(pending_with_satisfied.iter().any(|t| t.task_id == load_id));

    let deferred = Database::create_deferred_job(
        db,
        extract_id,
        "custom_http",
        "edge-job-1",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    assert!(deferred.started_at.is_none());
    Database::update_deferred_job_status(db, deferred.id, "polling", None).await?;
    let deferred_polled = Database::get_deferred_jobs_for_task(db, extract_id).await?;
    assert!(
        deferred_polled
            .iter()
            .any(|j| j.id == deferred.id && j.started_at.is_some())
    );

    let deferred_completed = Database::create_deferred_job(
        db,
        extract_id,
        "custom_http",
        "edge-job-2",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::complete_deferred_job(db, deferred_completed.id).await?;
    let deferred_failed = Database::create_deferred_job(
        db,
        extract_id,
        "custom_http",
        "edge-job-3",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::fail_deferred_job(db, deferred_failed.id, "failed").await?;

    let deferred_cancelled = Database::create_deferred_job(
        db,
        extract_id,
        "custom_http",
        "edge-job-4",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::cancel_deferred_jobs_for_task(db, extract_id).await?;
    let deferred_after = Database::get_deferred_jobs_for_task(db, extract_id).await?;
    assert!(deferred_after.iter().any(|j| j.id == deferred_completed.id
        && j.started_at.is_some()
        && j.finished_at.is_some()));
    assert!(
        deferred_after.iter().any(|j| j.id == deferred_failed.id
            && j.started_at.is_some()
            && j.finished_at.is_some())
    );
    assert!(
        deferred_after
            .iter()
            .any(|j| j.id == deferred_cancelled.id && j.status_str() == "cancelled")
    );

    let pending_deferred = Database::get_pending_deferred_jobs(db).await?;
    assert!(pending_deferred.iter().all(|j| j.task_id != extract_id));

    Database::update_run_status(db, run_id, "failed", Some("edge fail")).await?;
    let run_failed = Database::get_run(db, run_id).await?;
    assert_eq!(run_failed.status_str(), "failed");
    assert!(run_failed.finished_at.is_some());
    assert_eq!(run_failed.error.as_deref(), Some("edge fail"));

    Database::delete_workflow(db, "pg-edges").await?;
    assert!(Database::get_workflow(db, "pg-edges").await.is_err());

    ctx.cleanup().await
}
