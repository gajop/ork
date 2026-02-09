use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ork_core::database::{RunListQuery, WorkflowListQuery};
use ork_core::models::{RunStatus, TaskStatus};

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
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);
    let status_filter = match params.status.as_deref() {
        Some(raw) => match RunStatus::parse(raw) {
            Some(status) => Some(status),
            None => {
                return Json(PaginatedRuns {
                    items: Vec::new(),
                    total: 0,
                    limit,
                    offset,
                    has_more: false,
                })
                .into_response();
            }
        },
        None => None,
    };
    let query = RunListQuery {
        limit,
        offset,
        status: status_filter,
        workflow_name: params.workflow,
    };
    let page = match api.db.list_runs_page(&query).await {
        Ok(page) => page,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let items: Vec<RunListItem> = page
        .items
        .into_iter()
        .map(|entry| RunListItem {
            id: entry.run.id.to_string(),
            workflow: entry
                .workflow_name
                .unwrap_or_else(|| entry.run.workflow_id.to_string()),
            status: entry.run.status().as_str().to_string(),
            created_at: fmt_time(entry.run.created_at),
            started_at: entry.run.started_at.map(fmt_time),
            finished_at: entry.run.finished_at.map(fmt_time),
        })
        .collect();

    let has_more = offset + items.len() < page.total;

    Json(PaginatedRuns {
        items,
        total: page.total,
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
    let limit = params.limit.unwrap_or(100).min(200);
    let offset = params.offset.unwrap_or(0);
    let query = WorkflowListQuery {
        limit,
        offset,
        search: params.search,
    };
    let page = match api.db.list_workflows_page(&query).await {
        Ok(page) => page,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let items: Vec<WorkflowListItem> = page
        .items
        .into_iter()
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

    let has_more = offset + items.len() < page.total;

    Json(PaginatedWorkflows {
        items,
        total: page.total,
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
                status: run.status().as_str().to_string(),
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

pub(crate) async fn cancel_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match api.db.cancel_run(run_id).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: RunStatus::Cancelled.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status() {
        RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled => {
            return StatusCode::CONFLICT.into_response();
        }
        RunStatus::Paused => return StatusCode::OK.into_response(),
        _ => {}
    }
    match api
        .db
        .update_run_status(run_id, RunStatus::Paused, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: RunStatus::Paused.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_run(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let run_id = match Uuid::parse_str(&id) {
        Ok(rid) => rid,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let run = match api.db.get_run(run_id).await {
        Ok(run) => run,
        Err(err) if is_not_found(&err) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match run.status() {
        RunStatus::Paused => {}
        RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled => {
            return StatusCode::CONFLICT.into_response();
        }
        _ => return StatusCode::CONFLICT.into_response(),
    }
    let tasks = match api.db.list_tasks(run_id).await {
        Ok(tasks) => tasks,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let new_status = if tasks.is_empty() {
        RunStatus::Pending
    } else {
        RunStatus::Running
    };
    match api.db.update_run_status(run_id, new_status, None).await {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::RunUpdated {
                run_id: id.clone(),
                status: new_status.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn pause_task(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
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
    match task.status() {
        TaskStatus::Pending => {}
        TaskStatus::Paused => return StatusCode::OK.into_response(),
        TaskStatus::Dispatched | TaskStatus::Running => {
            return StatusCode::CONFLICT.into_response();
        }
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api
        .db
        .update_task_status(task_id, TaskStatus::Paused, None, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: TaskStatus::Paused.as_str().to_string(),
            });
            StatusCode::OK.into_response()
        }
        Err(err) if is_not_found(&err) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(crate) async fn resume_task(
    State(api): State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
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
    match task.status() {
        TaskStatus::Paused => {}
        _ => return StatusCode::CONFLICT.into_response(),
    }
    match api
        .db
        .update_task_status(task_id, TaskStatus::Pending, None, None)
        .await
    {
        Ok(_) => {
            let _ = api.broadcast_tx.send(StateUpdate::TaskUpdated {
                run_id: run_id.to_string(),
                task_id: id.clone(),
                status: TaskStatus::Pending.as_str().to_string(),
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

    if api
        .db
        .update_workflow_schedule(workflow.id, payload.schedule.as_deref(), payload.enabled)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    StatusCode::OK.into_response()
}
