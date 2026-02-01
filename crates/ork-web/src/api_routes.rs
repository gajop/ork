use std::collections::{HashMap, HashSet};
use axum::{
    Json,
    extract::{Path, Query, State, ws::{WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::Html,
    response::IntoResponse,
};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ork_core::database::Database;

use crate::handlers::{WorkflowInfo, WorkflowTaskInfo};

use crate::api::{ApiServer, StateUpdate, fmt_time, is_not_found};

#[derive(Deserialize)]
pub(crate) struct RunQueryParams {
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

pub(crate) async fn list_runs(
    State(api): State<ApiServer>,
    Query(params): Query<RunQueryParams>,
) -> impl IntoResponse {
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
    })
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct WorkflowQueryParams {
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

pub(crate) async fn list_workflows(
    State(api): State<ApiServer>,
    Query(params): Query<WorkflowQueryParams>,
) -> impl IntoResponse {
    let mut workflows = match api.db.list_workflows().await {
        Ok(w) => w,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    workflows.sort_by(|a, b| a.name.cmp(&b.name));

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
    })
    .into_response()
}

#[derive(Deserialize)]
pub(crate) struct StartRunRequest {
    workflow: String,
}

pub(crate) async fn start_run(
    State(api): State<ApiServer>,
    Json(payload): Json<StartRunRequest>,
) -> impl IntoResponse {
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
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: run.id.to_string(),
                status: run.status_str().to_string(),
            });
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

pub(crate) async fn workflow_detail(
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

pub(crate) async fn cancel_run(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match api.db.cancel_run(run_id).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: "cancelled".to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_run(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status_str() {
        "success" | "failed" | "cancelled" => return StatusCode::CONFLICT.into_response(),
        "paused" => return StatusCode::OK.into_response(),
        _ => {}
    }
    match api.db.update_run_status(run_id, "paused", None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: "paused".to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_run(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status_str() {
        "paused" => {}
        "success" | "failed" | "cancelled" => return StatusCode::CONFLICT.into_response(),
        _ => return StatusCode::CONFLICT.into_response(),
    }
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let new_status = if tasks.is_empty() { "pending" } else { "running" };
    match api.db.update_run_status(run_id, new_status, None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: new_status.to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_task(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&id) {
        Ok(tid) => tid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let (run_id, _) = match api.db.get_task_identity(task_id).await {
        Ok(val) => val,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task = match tasks.into_iter().find(|t| t.id == task_id) {
        Some(task) => task,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match task.status_str() {
        "pending" => {}
        "paused" => return StatusCode::OK.into_response(),
        "dispatched" | "running" => return StatusCode::CONFLICT.into_response(),
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api.db.update_task_status(task_id, "paused", None, None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: "paused".to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_task(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
    let task_id = match Uuid::parse_str(&id) {
        Ok(tid) => tid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let (run_id, _) = match api.db.get_task_identity(task_id).await {
        Ok(val) => val,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let task = match tasks.into_iter().find(|t| t.id == task_id) {
        Some(task) => task,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    match task.status_str() {
        "paused" => {}
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api.db.update_task_status(task_id, "pending", None, None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: "pending".to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct UpdateScheduleRequest {
    schedule: Option<String>,
    enabled: bool,
}

pub(crate) async fn update_workflow_schedule(
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

pub(crate) async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(api): State<ApiServer>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, api))
}

async fn websocket_connection(socket: WebSocket, api: ApiServer) {
    use axum::extract::ws::Message;

    let (mut sender, mut receiver) = socket.split();
    let mut rx = api.broadcast_tx.subscribe();

    let mut send_task = tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            let json = serde_json::to_string(&update).unwrap_or_default();
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_msg)) = receiver.next().await {
            // Client messages ignored for now (could add ping/pong here)
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}

pub(crate) async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
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
