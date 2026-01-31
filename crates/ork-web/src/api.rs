use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use uuid::Uuid;

use ork_core::database::Database;
use ork_core::models::json_inner;

#[derive(Clone)]
pub struct ApiServer {
    db: Arc<dyn Database>,
}

impl ApiServer {
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    pub async fn serve(self, addr: SocketAddr) -> JoinHandle<()> {
        let cors = tower_http::cors::CorsLayer::very_permissive();
        let router = Router::new()
            .route("/", get(ui))
            .route(
                "/api/runs",
                get(list_runs).post(axum::routing::post(start_run)),
            )
            .route("/api/runs/:id", get(run_detail))
            .route("/api/workflows", get(list_workflows))
            .route("/api/workflows/:name", get(workflow_detail))
            .with_state(self)
            .layer(cors);

        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .expect("bind address");
            axum::serve(listener, router).await.expect("server error");
        })
    }
}

#[derive(Serialize)]
struct RunListItem {
    id: String,
    workflow: String,
    status: String,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
}

async fn list_runs(State(api): State<ApiServer>) -> impl IntoResponse {
    let mut runs = match api.db.list_runs(None).await {
        Ok(r) => r,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let workflow_ids: HashSet<Uuid> = runs.iter().map(|r| r.workflow_id).collect();
    let workflow_map = match load_workflow_names(&*api.db, &workflow_ids).await {
        Ok(map) => map,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let items: Vec<RunListItem> = runs
        .into_iter()
        .take(50)
        .map(|r| RunListItem {
            id: r.id.to_string(),
            workflow: workflow_map
                .get(&r.workflow_id)
                .cloned()
                .unwrap_or_else(|| r.workflow_id.to_string()),
            status: r.status_str().to_string(),
            created_at: fmt_time(r.created_at),
            started_at: r.started_at.map(fmt_time),
            finished_at: r.finished_at.map(fmt_time),
        })
        .collect();
    Json(items).into_response()
}

#[derive(Serialize)]
struct WorkflowListItem {
    id: String,
    name: String,
    executor: String,
    project: String,
    region: String,
}

async fn list_workflows(State(api): State<ApiServer>) -> impl IntoResponse {
    let mut workflows = match api.db.list_workflows().await {
        Ok(w) => w,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    workflows.sort_by(|a, b| a.name.cmp(&b.name));

    let items: Vec<WorkflowListItem> = workflows
        .into_iter()
        .map(|wf| WorkflowListItem {
            id: wf.id.to_string(),
            name: wf.name,
            executor: wf.executor_type,
            project: wf.project,
            region: wf.region,
        })
        .collect();
    Json(items).into_response()
}

#[derive(Serialize)]
struct RunDetail {
    run: RunInfo,
    tasks: Vec<TaskInfo>,
    workflow: Option<WorkflowInfo>,
}

#[derive(Serialize)]
struct RunInfo {
    id: String,
    workflow: String,
    status: String,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct TaskInfo {
    id: String,
    name: String,
    status: String,
    executor: String,
    depends_on: Vec<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    dispatched_at: Option<String>,
    output: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct StartRunRequest {
    workflow: String,
}

async fn start_run(State(api): State<ApiServer>, Json(payload): Json<StartRunRequest>) -> impl IntoResponse {
    let workflow = match api.db.get_workflow(&payload.workflow).await {
        Ok(wf) => wf,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match api.db.create_run(workflow.id, "ui").await {
        Ok(run) => {
            #[derive(Serialize)]
            struct Resp {
                run_id: String,
            }
            Json(Resp {
                run_id: run.id.to_string(),
            })
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Serialize, Clone)]
struct WorkflowInfo {
    name: String,
    tasks: Vec<WorkflowTaskInfo>,
}

#[derive(Serialize, Clone)]
struct WorkflowTaskInfo {
    name: String,
    depends_on: Vec<String>,
    executor: String,
}

async fn workflow_detail(
    Path(name): Path<String>,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    let workflow = match api.db.get_workflow(&name).await {
        Ok(wf) => wf,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut tasks = match api.db.list_workflow_tasks(workflow.id).await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    tasks.sort_by(|a, b| a.task_index.cmp(&b.task_index));
    let tasks = tasks
        .into_iter()
        .map(|task| WorkflowTaskInfo {
            name: task.task_name,
            depends_on: task.depends_on,
            executor: task.executor_type,
        })
        .collect();
    Json(WorkflowInfo {
        name: workflow.name,
        tasks,
    })
    .into_response()
}

async fn run_detail(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) => {
            if is_not_found(&err) {
                return StatusCode::NOT_FOUND.into_response();
            }
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let workflow = api.db.get_workflow_by_id(run.workflow_id).await.ok();
    let workflow_name = workflow
        .as_ref()
        .map(|wf| wf.name.clone())
        .unwrap_or_else(|| run.workflow_id.to_string());

    let mut task_rows = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    task_rows.sort_by(|a, b| a.task_index.cmp(&b.task_index));

    let tasks: Vec<TaskInfo> = task_rows
        .into_iter()
        .map(|task| TaskInfo {
            id: task.id.to_string(),
            name: task.task_name,
            status: task.status_str().to_string(),
            executor: task.executor_type,
            depends_on: task.depends_on,
            started_at: task.started_at.map(fmt_time),
            finished_at: task.finished_at.map(fmt_time),
            dispatched_at: task.dispatched_at.map(fmt_time),
            output: task.output.as_ref().map(|out| json_inner(out).clone()),
            error: task.error,
        })
        .collect();

    let workflow_info = if let Some(workflow) = workflow.as_ref() {
        let mut workflow_tasks = api
            .db
            .list_workflow_tasks(workflow.id)
            .await
            .unwrap_or_default();
        workflow_tasks.sort_by(|a, b| a.task_index.cmp(&b.task_index));
        Some(WorkflowInfo {
            name: workflow.name.clone(),
            tasks: workflow_tasks
                .into_iter()
                .map(|task| WorkflowTaskInfo {
                    name: task.task_name,
                    depends_on: task.depends_on,
                    executor: task.executor_type,
                })
                .collect(),
        })
    } else {
        None
    };

    Json(RunDetail {
        run: RunInfo {
            id: run.id.to_string(),
            workflow: workflow_name,
            status: run.status_str().to_string(),
            created_at: fmt_time(run.created_at),
            started_at: run.started_at.map(fmt_time),
            finished_at: run.finished_at.map(fmt_time),
            error: run.error,
        },
        tasks,
        workflow: workflow_info,
    })
    .into_response()
}

async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

fn fmt_time(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339()
}

async fn load_workflow_names(
    db: &dyn Database,
    workflow_ids: &HashSet<Uuid>,
) -> anyhow::Result<HashMap<Uuid, String>> {
    if workflow_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids: Vec<Uuid> = workflow_ids.iter().cloned().collect();
    let workflows = db.get_workflows_by_ids(&ids).await?;
    let map = workflows.into_iter().map(|wf| (wf.id, wf.name)).collect();
    Ok(map)
}

fn is_not_found(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("row not found") || msg.contains("no rows") || msg.contains("not found")
}
