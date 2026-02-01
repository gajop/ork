use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;
use serde_json::{Value, json};
use std::sync::Arc;
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

async fn request_json(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("json")
    };
    (status, json)
}

async fn create_workflow_with_tasks(db: &SqliteDatabase, name: &str) -> ork_core::models::Workflow {
    let workflow = db
        .create_workflow(name, None, "dag", "local", "local", "dag", None, None)
        .await
        .expect("workflow");
    let tasks = vec![
        NewWorkflowTask {
            task_index: 0,
            task_name: "first".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: json!({"command": "echo first"}),
        },
        NewWorkflowTask {
            task_index: 1,
            task_name: "second".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["first".to_string()],
            params: json!({"command": "echo second"}),
        },
    ];
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
    let run = db
        .create_run(workflow.id, "test")
        .await
        .expect("run");
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
async fn test_workflow_endpoints() {
    let (app, db) = setup().await;

    let yaml = r#"
name: api_workflow
tasks:
  first:
    executor: process
    command: "echo first"
  second:
    executor: process
    command: "echo second"
    depends_on: [first]
"#;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/workflows",
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "api_workflow");

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/workflows?limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["total"].as_u64().unwrap_or(0) >= 1);

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/workflows/api_workflow",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "api_workflow");
    assert_eq!(body["tasks"].as_array().unwrap().len(), 2);

    let (status, _) = request_json(
        &app,
        Method::PATCH,
        "/api/workflows/api_workflow/schedule",
        Some(json!({ "schedule": "*/5 * * * *", "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let updated = db.get_workflow("api_workflow").await.expect("workflow");
    assert_eq!(updated.schedule.as_deref(), Some("*/5 * * * *"));
    assert!(updated.schedule_enabled);
}

#[tokio::test]
async fn test_run_list_and_detail_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "runs_workflow").await;

    let (status, body) = request_json(
        &app,
        Method::POST,
        "/api/runs",
        Some(json!({ "workflow": "runs_workflow" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run_id = body["run_id"].as_str().unwrap().to_string();
    let run_uuid = Uuid::parse_str(&run_id).expect("run id");

    create_run_with_tasks(&db, &workflow, 2).await;
    db.update_run_status(run_uuid, "running", None)
        .await
        .expect("run status");

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["total"].as_u64().unwrap_or(0) >= 2);

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?status=running&limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["status"], "running");
    }

    let (status, body) = request_json(
        &app,
        Method::GET,
        "/api/runs?workflow=runs_workflow&limit=10&offset=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for item in body["items"].as_array().unwrap() {
        assert_eq!(item["workflow"], "runs_workflow");
    }

    let (status, body) = request_json(
        &app,
        Method::GET,
        &format!("/api/runs/{}", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["run"]["status"], "running");
}

#[tokio::test]
async fn test_run_cancel_pause_resume_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "control_workflow").await;
    let run_id = create_run_with_tasks(&db, &workflow, 2).await;
    db.update_run_status(run_id, "running", None)
        .await
        .expect("run status");

    let tasks = db.list_tasks(run_id).await.expect("tasks");
    db.update_task_status(tasks[0].id, "running", None, None)
        .await
        .expect("task status");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/pause", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "paused");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/resume", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "running");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/runs/{}/cancel", run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run = db.get_run(run_id).await.expect("run");
    assert_eq!(run.status_str(), "cancelled");
    let tasks = db.list_tasks(run_id).await.expect("tasks");
    for task in tasks {
        assert_eq!(task.status_str(), "cancelled");
    }
}

#[tokio::test]
async fn test_task_pause_resume_endpoints() {
    let (app, db) = setup().await;
    let workflow = create_workflow_with_tasks(&db, "task_control").await;
    let run_id = create_run_with_tasks(&db, &workflow, 1).await;

    let task = db.list_tasks(run_id).await.expect("tasks").into_iter().next().unwrap();

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/pause", task.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let task = db.list_tasks(run_id).await.expect("tasks").into_iter().next().unwrap();
    assert_eq!(task.status_str(), "paused");

    let (status, _) = request_json(
        &app,
        Method::POST,
        &format!("/api/tasks/{}/resume", task.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let task = db.list_tasks(run_id).await.expect("tasks").into_iter().next().unwrap();
    assert_eq!(task.status_str(), "pending");
}
