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

async fn create_workflow_and_run(db: &PostgresDatabase, name: &str) -> Result<(Uuid, Uuid)> {
    let wf = Database::create_workflow(
        db,
        name,
        Some("postgres coverage"),
        "dag",
        "local",
        "local",
        "dag",
        Some(serde_json::json!({"task_count": 2})),
        Some("*/5 * * * *"),
    )
    .await?;
    let run = Database::create_run(db, wf.id, "test").await?;
    Ok((wf.id, run.id))
}

#[tokio::test]
async fn test_postgres_workflow_run_task_contract() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let (workflow_id, run_id) = create_workflow_and_run(db, "pg-contract").await?;

    let wf = Database::get_workflow(db, "pg-contract").await?;
    assert_eq!(wf.id, workflow_id);
    assert_eq!(wf.schedule.as_deref(), Some("*/5 * * * *"));

    let all = Database::list_workflows(db).await?;
    assert!(all.iter().any(|w| w.id == workflow_id));
    let by_id = Database::get_workflow_by_id(db, workflow_id).await?;
    assert_eq!(by_id.name, "pg-contract");
    let by_ids = Database::get_workflows_by_ids(db, &[workflow_id]).await?;
    assert_eq!(by_ids.len(), 1);

    Database::create_workflow_tasks(
        db,
        workflow_id,
        &[
            NewWorkflowTask {
                task_index: 0,
                task_name: "a".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo a"}),
                signature: Some(serde_json::json!({"input":"str"})),
            },
            NewWorkflowTask {
                task_index: 1,
                task_name: "b".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["a".to_string()],
                params: serde_json::json!({"command":"echo b"}),
                signature: None,
            },
        ],
    )
    .await?;
    let workflow_tasks = Database::list_workflow_tasks(db, workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    Database::batch_create_tasks(db, run_id, 2, "pg-contract", "process").await?;
    let mut run_tasks = Database::list_tasks(db, run_id).await?;
    assert_eq!(run_tasks.len(), 2);
    run_tasks.sort_by_key(|t| t.task_index);

    let first_task = run_tasks[0].id;
    let second_task = run_tasks[1].id;

    Database::update_run_status(db, run_id, "running", None).await?;
    Database::update_task_status(db, first_task, "dispatched", Some("exec-1"), None).await?;
    Database::update_task_status(db, first_task, "running", Some("exec-1"), None).await?;
    Database::append_task_log(db, first_task, "line-1\n").await?;
    Database::update_task_output(db, first_task, serde_json::json!({"ok": true})).await?;
    Database::update_task_status(db, first_task, "success", Some("exec-1"), None).await?;

    let outputs = Database::get_task_outputs(db, run_id, &[String::from("task_0")]).await?;
    assert!(outputs.contains_key("task_0"));
    let retry_meta = Database::get_task_retry_meta(db, &[first_task, second_task]).await?;
    assert_eq!(retry_meta.len(), 2);
    assert_eq!(Database::get_task_run_id(db, first_task).await?, run_id);
    let identity = Database::get_task_identity(db, first_task).await?;
    assert_eq!(identity.0, run_id);
    assert_eq!(identity.1, "task_0");

    let pending = Database::get_pending_tasks(db).await?;
    assert!(pending.iter().any(|t| t.id == second_task));
    let pending_with_wf = Database::get_pending_tasks_with_workflow(db, 50).await?;
    assert!(pending_with_wf.iter().any(|t| t.task_id == second_task));

    Database::batch_update_task_status(
        db,
        &[(second_task, "failed", Some("exec-2"), Some("batch failure"))],
    )
    .await?;
    let stats = Database::get_run_task_stats(db, run_id).await?;
    assert_eq!(stats.0, 2);
    assert_eq!(stats.1, 2);
    assert_eq!(stats.2, 1);

    let retry_at = Utc::now() + Duration::seconds(1);
    Database::reset_task_for_retry(db, second_task, Some("retry me"), Some(retry_at)).await?;
    let pending_again = Database::get_pending_tasks(db).await?;
    assert!(pending_again.iter().any(|t| t.id == second_task));

    Database::update_workflow_schedule(db, workflow_id, Some("*/5 * * * *"), true).await?;
    let due_before = Database::get_due_scheduled_workflows(db).await?;
    assert!(due_before.iter().any(|w| w.id == workflow_id));
    Database::update_workflow_schedule_times(
        db,
        workflow_id,
        Utc::now(),
        Some(Utc::now() + Duration::hours(1)),
    )
    .await?;
    Database::update_workflow_schedule(db, workflow_id, Some("0 * * * *"), true).await?;
    let due_after = Database::get_due_scheduled_workflows(db).await?;
    assert!(!due_after.iter().any(|w| w.id == workflow_id));

    Database::cancel_run(db, run_id).await?;
    let run = Database::get_run(db, run_id).await?;
    assert_eq!(run.status_str(), "cancelled");
    let listed_runs = Database::list_runs(db, Some(workflow_id)).await?;
    assert!(!listed_runs.is_empty());
    let pending_runs = Database::get_pending_runs(db).await?;
    assert!(pending_runs.iter().all(|r| r.id != run_id));

    Database::delete_workflow(db, "pg-contract").await?;
    assert!(Database::get_workflow(db, "pg-contract").await.is_err());

    ctx.cleanup().await
}

#[tokio::test]
async fn test_postgres_dag_dependency_and_deferred_jobs_contract() -> Result<()> {
    let Some(ctx) = PostgresTestContext::setup().await? else {
        return Ok(());
    };
    let db = &ctx.db;

    let (workflow_id, run_id) = create_workflow_and_run(db, "pg-deferrables").await?;

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
                max_retries: 1,
                timeout_seconds: Some(60),
            },
            NewTask {
                task_index: 1,
                task_name: "load".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec!["extract".to_string()],
                params: serde_json::json!({"command":"echo load"}),
                max_retries: 0,
                timeout_seconds: Some(60),
            },
        ],
    )
    .await?;

    Database::update_run_status(db, run_id, "running", None).await?;

    let tasks = Database::list_tasks(db, run_id).await?;
    let extract_id = tasks
        .iter()
        .find(|t| t.task_name == "extract")
        .expect("extract task")
        .id;
    let load_id = tasks
        .iter()
        .find(|t| t.task_name == "load")
        .expect("load task")
        .id;

    let failed = Database::mark_tasks_failed_by_dependency(
        db,
        run_id,
        &[String::from("extract")],
        "extract failed",
    )
    .await?;
    assert_eq!(failed, vec![String::from("load")]);

    let running_before = Database::get_running_tasks(db).await?;
    assert!(running_before.is_empty());
    Database::update_task_status(db, extract_id, "running", Some("exec-3"), None).await?;
    let running_after = Database::get_running_tasks(db).await?;
    assert!(running_after.iter().any(|t| t.id == extract_id));

    let job = Database::create_deferred_job(
        db,
        extract_id,
        "custom_http",
        "job-1",
        serde_json::json!({"url":"http://example/status","completion_field":"state","completion_value":"done"}),
    )
    .await?;
    assert_eq!(job.status_str(), "pending");

    let pending_jobs = Database::get_pending_deferred_jobs(db).await?;
    assert!(pending_jobs.iter().any(|j| j.id == job.id));
    Database::update_deferred_job_status(db, job.id, "polling", None).await?;
    Database::update_deferred_job_polled(db, job.id).await?;
    let by_task = Database::get_deferred_jobs_for_task(db, extract_id).await?;
    assert_eq!(by_task.len(), 1);
    assert_eq!(by_task[0].status_str(), "polling");

    Database::complete_deferred_job(db, job.id).await?;
    let completed = Database::get_deferred_jobs_for_task(db, extract_id).await?;
    assert_eq!(completed[0].status_str(), "completed");

    let failed_job = Database::create_deferred_job(
        db,
        load_id,
        "custom_http",
        "job-2",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::fail_deferred_job(db, failed_job.id, "bad gateway").await?;

    let cancel_job = Database::create_deferred_job(
        db,
        load_id,
        "custom_http",
        "job-3",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::update_deferred_job_status(db, cancel_job.id, "polling", None).await?;
    Database::cancel_deferred_jobs_for_task(db, load_id).await?;
    let load_jobs = Database::get_deferred_jobs_for_task(db, load_id).await?;
    assert!(load_jobs.iter().any(|j| j.status_str() == "failed"));
    assert!(load_jobs.iter().any(|j| j.status_str() == "cancelled"));

    Database::delete_workflow(db, "pg-deferrables").await?;
    assert!(Database::get_workflow_by_id(db, workflow_id).await.is_err());

    ctx.cleanup().await
}
