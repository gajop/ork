use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::{ApiServer, fmt_time, is_not_found};
use crate::workflow_tasks::build_workflow_tasks;
use ork_core::models::json_inner;

#[derive(Deserialize)]
pub struct CreateWorkflowRequest {
    pub yaml: String,
    pub project: Option<String>,
    pub region: Option<String>,
    pub root: Option<String>,
    pub replace: Option<bool>,
}

pub async fn create_workflow(
    State(api): State<ApiServer>,
    Json(payload): Json<CreateWorkflowRequest>,
) -> impl IntoResponse {
    let project = payload.project.unwrap_or_else(|| "local".to_string());
    let region = payload.region.unwrap_or_else(|| "local".to_string());
    let root = payload
        .root
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let root = root.canonicalize().unwrap_or(root);

    let definition: ork_core::workflow::Workflow = match serde_yaml::from_str(&payload.yaml) {
        Ok(def) => def,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if let Err(err) = definition.validate() {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }
    let compiled = match definition.compile(&root) {
        Ok(compiled) => compiled,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };

    let replace = payload.replace.unwrap_or(true);
    let workflow = match api.db.get_workflow(&definition.name).await {
        Ok(existing) => {
            if !replace {
                return (
                    StatusCode::CONFLICT,
                    "Workflow already exists; pass replace=true to update tasks.".to_string(),
                )
                    .into_response();
            }
            existing
        }
        Err(err) => {
            if is_not_found(&err) {
                match api
                    .db
                    .create_workflow(
                        &definition.name,
                        None,
                        "dag",
                        &region,
                        &project,
                        "dag",
                        None,
                        definition.schedule.as_deref(),
                    )
                    .await
                {
                    Ok(wf) => wf,
                    Err(err) => {
                        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
                            .into_response();
                    }
                }
            } else {
                return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
            }
        }
    };

    let workflow_tasks = build_workflow_tasks(&compiled);
    if let Err(err) = api
        .db
        .create_workflow_tasks(workflow.id, &workflow_tasks)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }

    #[derive(Serialize)]
    struct Resp {
        name: String,
        id: String,
    }
    Json(Resp {
        name: workflow.name,
        id: workflow.id.to_string(),
    })
    .into_response()
}

#[derive(Serialize)]
pub struct RunDetail {
    pub run: RunInfo,
    pub tasks: Vec<TaskInfo>,
    pub workflow: Option<WorkflowInfo>,
}

#[derive(Serialize)]
pub struct RunInfo {
    pub id: String,
    pub workflow: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub executor: String,
    pub depends_on: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub dispatched_at: Option<String>,
    pub output: Option<serde_json::Value>,
    pub logs: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct WorkflowInfo {
    pub name: String,
    pub tasks: Vec<WorkflowTaskInfo>,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub next_scheduled_at: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct WorkflowTaskInfo {
    pub name: String,
    pub depends_on: Vec<String>,
    pub executor: String,
}

pub async fn run_detail(State(api): State<ApiServer>, Path(id): Path<String>) -> impl IntoResponse {
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
        .map(|task| {
            let status = task.status_str().to_string();
            TaskInfo {
                id: task.id.to_string(),
                name: task.task_name,
                status,
                executor: task.executor_type,
                depends_on: task.depends_on,
                started_at: task.started_at.map(fmt_time),
                finished_at: task.finished_at.map(fmt_time),
                dispatched_at: task.dispatched_at.map(fmt_time),
                output: task.output.as_ref().map(|out| json_inner(out).clone()),
                logs: task.logs,
                error: task.error,
            }
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
            schedule: workflow.schedule.clone(),
            schedule_enabled: workflow.schedule_enabled,
            next_scheduled_at: workflow.next_scheduled_at.map(fmt_time),
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
