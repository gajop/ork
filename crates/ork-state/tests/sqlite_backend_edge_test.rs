#![cfg(feature = "sqlite")]

use anyhow::Result;
use chrono::{Duration, Utc};
use ork_core::database::{Database, NewTask, NewWorkflowTask};
use ork_state::SqliteDatabase;
use uuid::Uuid;

fn task_id(tasks: &[ork_core::models::Task], name: &str) -> Uuid {
    tasks
        .iter()
        .find(|t| t.task_name == name)
        .unwrap_or_else(|| panic!("missing task: {name}"))
        .id
}

#[tokio::test]
async fn test_sqlite_branch_and_edge_paths() -> Result<()> {
    let db = SqliteDatabase::new(":memory:").await?;
    Database::run_migrations(&db).await?;

    let wf = Database::create_workflow(
        &db,
        "sqlite-edges",
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

    Database::create_workflow_tasks(
        &db,
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
        &db,
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
    let workflow_tasks = Database::list_workflow_tasks(&db, workflow_id).await?;
    assert_eq!(workflow_tasks.len(), 2);

    let run = Database::create_run(&db, workflow_id, "tests").await?;
    let run_id = run.id;
    Database::batch_create_dag_tasks(
        &db,
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

    let pending = Database::get_pending_tasks(&db).await?;
    assert!(pending.iter().any(|t| t.task_name == "extract"));

    Database::batch_update_task_status(&db, &[]).await?;
    let no_failed =
        Database::mark_tasks_failed_by_dependency(&db, run_id, &[], "should not apply").await?;
    assert!(no_failed.is_empty());

    let empty_outputs = Database::get_task_outputs(&db, run_id, &[]).await?;
    assert!(empty_outputs.is_empty());
    let empty_retry = Database::get_task_retry_meta(&db, &[]).await?;
    assert!(empty_retry.is_empty());

    let tasks = Database::list_tasks(&db, run_id).await?;
    let extract_id = task_id(&tasks, "extract");
    let load_id = task_id(&tasks, "load");

    Database::update_task_output(&db, extract_id, serde_json::json!({"ok": true})).await?;
    let outputs = Database::get_task_outputs(&db, run_id, &[String::from("extract")]).await?;
    assert_eq!(outputs.get("extract"), Some(&serde_json::json!({"ok": true})));

    let retry_at = Utc::now() + Duration::seconds(30);
    Database::reset_task_for_retry(&db, extract_id, Some("retry"), Some(retry_at)).await?;
    Database::update_task_status(&db, extract_id, "success", Some("exec-edge"), None).await?;
    Database::update_run_status(&db, run_id, "running", None).await?;
    let pending_with_satisfied = Database::get_pending_tasks_with_workflow(&db, 10).await?;
    assert!(pending_with_satisfied.iter().any(|t| t.task_id == load_id));

    let deferred = Database::create_deferred_job(
        &db,
        extract_id,
        "custom_http",
        "edge-job-1",
        serde_json::json!({"url":"http://example/status"}),
    )
    .await?;
    Database::get_deferred_jobs_for_task(&db, extract_id).await?;
    Database::cancel_deferred_jobs_for_task(&db, extract_id).await?;
    let deferred_after = Database::get_deferred_jobs_for_task(&db, extract_id).await?;
    assert!(
        deferred_after
            .iter()
            .any(|j| j.id == deferred.id && j.status_str() == "cancelled")
    );

    Ok(())
}
