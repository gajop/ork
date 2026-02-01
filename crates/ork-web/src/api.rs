use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
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

use crate::handlers::{self, WorkflowInfo, WorkflowTaskInfo};

#[derive(Clone)]
pub struct ApiServer {
    pub db: Arc<dyn Database>,
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
            .route("/api/runs/{id}", get(handlers::run_detail))
            .route("/api/runs/{id}/cancel", axum::routing::post(cancel_run))
            .route("/api/workflows", get(list_workflows).post(handlers::create_workflow))
            .route("/api/workflows/{name}", get(workflow_detail))
            .route("/api/workflows/{name}/schedule", axum::routing::patch(update_workflow_schedule))
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

#[derive(Deserialize)]
struct RunQueryParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    workflow: Option<String>,
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

#[derive(Serialize)]
struct PaginatedRuns {
    items: Vec<RunListItem>,
    total: usize,
    limit: usize,
    offset: usize,
    has_more: bool,
}

async fn list_runs(State(api): State<ApiServer>, Query(params): Query<RunQueryParams>) -> impl IntoResponse {
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

    // Apply filters
    if let Some(ref status_filter) = params.status {
        runs.retain(|r| r.status_str() == status_filter);
    }
    if let Some(ref workflow_filter) = params.workflow {
        runs.retain(|r| {
            workflow_map
                .get(&r.workflow_id)
                .map(|name| name == workflow_filter)
                .unwrap_or(false)
        });
    }

    let total = runs.len();
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let items: Vec<RunListItem> = runs
        .into_iter()
        .skip(offset)
        .take(limit)
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

    let has_more = offset + items.len() < total;

    Json(PaginatedRuns {
        items,
        total,
        limit,
        offset,
        has_more,
    }).into_response()
}

#[derive(Deserialize)]
struct WorkflowQueryParams {
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    search: Option<String>,
}

#[derive(Serialize)]
struct WorkflowListItem {
    id: String,
    name: String,
    executor: String,
    project: String,
    region: String,
    schedule: Option<String>,
    schedule_enabled: bool,
    next_scheduled_at: Option<String>,
}

#[derive(Serialize)]
struct PaginatedWorkflows {
    items: Vec<WorkflowListItem>,
    total: usize,
    limit: usize,
    offset: usize,
    has_more: bool,
}

async fn list_workflows(State(api): State<ApiServer>, Query(params): Query<WorkflowQueryParams>) -> impl IntoResponse {
    let mut workflows = match api.db.list_workflows().await {
        Ok(w) => w,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    workflows.sort_by(|a, b| a.name.cmp(&b.name));

    // Apply search filter
    if let Some(ref search_term) = params.search {
        let search_lower = search_term.to_lowercase();
        workflows.retain(|wf| wf.name.to_lowercase().contains(&search_lower));
    }

    let total = workflows.len();
    let limit = params.limit.unwrap_or(100).min(200);
    let offset = params.offset.unwrap_or(0);

    let items: Vec<WorkflowListItem> = workflows
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|wf| WorkflowListItem {
            id: wf.id.to_string(),
            name: wf.name,
            executor: wf.executor_type,
            project: wf.project,
            region: wf.region,
            schedule: wf.schedule,
            schedule_enabled: wf.schedule_enabled,
            next_scheduled_at: wf.next_scheduled_at.map(fmt_time),
        })
        .collect();

    let has_more = offset + items.len() < total;

    Json(PaginatedWorkflows {
        items,
        total,
        limit,
        offset,
        has_more,
    }).into_response()
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
        schedule: workflow.schedule,
        schedule_enabled: workflow.schedule_enabled,
        next_scheduled_at: workflow.next_scheduled_at.map(fmt_time),
    })
    .into_response()
}

async fn cancel_run(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match api.db.cancel_run(run_id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateScheduleRequest {
    schedule: Option<String>,
    enabled: bool,
}

async fn update_workflow_schedule(
    State(api): State<ApiServer>,
    Path(name): Path<String>,
    Json(payload): Json<UpdateScheduleRequest>,
) -> impl IntoResponse {
    let workflow = match api.db.get_workflow(&name).await {
        Ok(wf) => wf,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    if let Err(_) = api.db.update_workflow_schedule(
        workflow.id,
        payload.schedule.as_deref(),
        payload.enabled,
    ).await {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::OK.into_response()
}

async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

pub fn fmt_time(ts: DateTime<Utc>) -> String {
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

pub fn is_not_found(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("row not found") || msg.contains("no rows") || msg.contains("not found")
}
