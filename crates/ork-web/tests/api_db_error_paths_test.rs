use axum::body::{Body, Bytes, to_bytes};
use axum::http::{Method, Request, StatusCode};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use ork_core::database::{Database, NewTask, NewWorkflowTask};
use ork_state::SqliteDatabase;
use ork_web::api::{ApiServer, build_router};

async fn setup() -> (axum::Router, Arc<SqliteDatabase>) {
    let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("db"));
    db.run_migrations().await.expect("migrations");
    let app = build_router(ApiServer::new(db.clone()));
    (app, db)
}

async fn request_raw(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Bytes) {
    let mut builder = Request::builder().method(method).uri(path);
    let body = if let Some(payload) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(payload.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("request body"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, bytes)
}

async fn create_workflow_with_tasks(db: &SqliteDatabase, name: &str) -> ork_core::models::Workflow {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("workflow");
    let tasks = vec![NewWorkflowTask {
        task_index: 0,
        task_name: "first".to_string(),
        executor_type: "process".to_string(),
        depends_on: vec![],
        params: json!({"command": "echo first"}),
        signature: None,
    }];
    db.create_workflow_tasks(workflow.id, &tasks)
        .await
        .expect("workflow tasks");
    workflow
}

async fn create_run_with_tasks(
    db: &SqliteDatabase,
    workflow: &ork_core::models::Workflow,
    task_count: i32,
) -> Uuid {
    let run = db.create_run(workflow.id, "test").await.expect("run");
    let tasks = (0..task_count)
        .map(|idx| NewTask {
            task_index: idx,
            task_name: format!("task_{}", idx),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: json!({"command": "echo ok"}),
            max_retries: 0,
            timeout_seconds: Some(1),
        })
        .collect::<Vec<_>>();
    db.batch_create_dag_tasks(run.id, &tasks)
        .await
        .expect("run tasks");
    run.id
}

#[tokio::test]
async fn test_handlers_and_routes_return_internal_error_when_db_unavailable() {
    let (app, db) = setup().await;

    db.pool().close().await;

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({
            "yaml": "name: w\ntasks:\n  one:\n    executor: process\n    command: echo one\n"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) = request_raw(&app, Method::GET, "/api/runs?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) =
        request_raw(&app, Method::GET, "/api/workflows?limit=10&offset=0", None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "missing" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_create_workflow_returns_internal_error_when_workflow_tasks_table_missing() {
    let (app, db) = setup().await;
    sqlx::query("DROP TABLE workflow_tasks")
        .execute(db.pool())
        .await
        .expect("drop workflow_tasks");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({
            "yaml": "name: broken_wf\ntasks:\n  one:\n    executor: process\n    command: echo one\n"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_run_detail_returns_internal_error_when_tasks_query_fails() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "detail_error_wf").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;

    sqlx::query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");

    let (status, _) = request_raw(&app, Method::GET, &format!("/api/runs/{}", run_id), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_start_run_returns_internal_error_when_run_insert_fails() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "start_run_insert_error").await;

    sqlx::query("DROP TABLE runs")
        .execute(db.pool())
        .await
        .expect("drop runs");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "start_run_insert_error" })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_workflow_detail_returns_internal_error_paths() {
    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_detail_error").await;

    db.pool().close().await;
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_detail_error",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    let (app, db) = setup().await;
    let _workflow = create_workflow_with_tasks(&db, "workflow_detail_task_error").await;
    sqlx::query("DROP TABLE workflow_tasks")
        .execute(db.pool())
        .await
        .expect("drop workflow_tasks");
    let (status, _) = request_raw(
        &app,
        Method::GET,
        "/api/workflows/workflow_detail_task_error",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_pause_resume_run_internal_error_paths() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "pause_resume_error").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;
    db.update_run_status(run_id, "paused", None)
        .await
        .expect("pause run");

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    sqlx::query("DROP TABLE tasks")
        .execute(db.pool())
        .await
        .expect("drop tasks");
    let (status, _) = request_raw(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}
