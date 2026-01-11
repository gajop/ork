use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::Html,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use ork_core::{
    types::{Run, TaskRun},
    workflow::Workflow,
};
use ork_runner::LocalScheduler;
use ork_state::{FileStateStore, LocalObjectStore, ObjectStore};

#[derive(Clone)]
pub struct ApiServer {
    scheduler: LocalScheduler,
    object_store: LocalObjectStore,
}

impl ApiServer {
    pub fn new(
        state_dir: &std::path::Path,
        run_dir: &std::path::Path,
        max_parallel: usize,
    ) -> Self {
        let state = FileStateStore::new(state_dir);
        let object_store = Arc::new(LocalObjectStore::new(run_dir));
        let scheduler = LocalScheduler::with_state(
            Arc::new(state),
            object_store.clone(),
            run_dir,
            max_parallel,
        );
        Self {
            scheduler,
            object_store: (*object_store).clone(),
        }
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
            .route("/api/workflow", get(workflow_detail))
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

async fn list_runs(
    axum::extract::State(api): axum::extract::State<ApiServer>,
) -> impl IntoResponse {
    let mut runs = match api.scheduler.list_runs().await {
        Ok(r) => r,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    let items: Vec<RunListItem> = runs
        .into_iter()
        .take(50)
        .map(|r| RunListItem {
            id: r.id,
            workflow: r.workflow,
            status: format!("{:?}", r.status),
            created_at: r.created_at.format("%H:%M:%S").to_string(),
            started_at: r.started_at.map(|t| t.format("%H:%M:%S").to_string()),
            finished_at: r.finished_at.map(|t| t.format("%H:%M:%S").to_string()),
        })
        .collect();
    Json(items).into_response()
}

#[derive(Serialize)]
struct RunDetail {
    run: Run,
    tasks: Vec<TaskRunWithOutput>,
    workflow: Option<WorkflowInfo>,
}

#[derive(Serialize)]
struct TaskRunWithOutput {
    task: TaskRun,
    output: Option<serde_json::Value>,
    deps: Vec<String>,
}

#[derive(Deserialize)]
struct StartRunRequest {
    workflow: String,
}

async fn start_run(
    axum::extract::State(api): axum::extract::State<ApiServer>,
    Json(payload): Json<StartRunRequest>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&payload.workflow);
    match load_and_compile(path) {
        Ok((wf, compiled)) => match api.scheduler.run_compiled(wf, compiled).await {
            Ok(run) => {
                #[derive(Serialize)]
                struct Resp<'a> {
                    run_id: &'a str,
                }
                Json(Resp {
                    run_id: &run.run_id,
                })
                .into_response()
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(_) => StatusCode::BAD_REQUEST.into_response(),
    }
}

#[derive(Serialize, Clone)]
struct WorkflowInfo {
    name: String,
    tasks: Vec<WorkflowTask>,
}

#[derive(Serialize, Clone)]
struct WorkflowTask {
    name: String,
    depends_on: Vec<String>,
    executor: String,
}

async fn workflow_detail(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(path) = params.get("path") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok((wf, _)) = load_and_compile(std::path::Path::new(path)) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let tasks = wf
        .tasks
        .iter()
        .map(|(name, task)| WorkflowTask {
            name: name.clone(),
            depends_on: task.depends_on.clone(),
            executor: format!("{:?}", task.executor),
        })
        .collect();
    Json(WorkflowInfo {
        name: wf.name,
        tasks,
    })
    .into_response()
}

async fn run_detail(
    axum::extract::State(api): axum::extract::State<ApiServer>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(run) = api.scheduler.get_run(&id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let task_runs = api.scheduler.task_runs(&id).await.unwrap_or_default();
    let mut tasks = Vec::with_capacity(task_runs.len());
    for task in task_runs {
        let deps = api
            .object_store
            .read_spec(&id, &task.task)
            .await
            .ok()
            .map(|spec| spec.upstream.keys().cloned().collect())
            .unwrap_or_else(Vec::new);
        let output: Option<serde_json::Value> = api
            .object_store
            .read_output(&id, &task.task)
            .await
            .ok()
            .flatten();
        tasks.push(TaskRunWithOutput { task, output, deps });
    }
    tasks.sort_by(|a, b| {
        let start_a = a.task.started_at.unwrap_or(a.task.created_at);
        let start_b = b.task.started_at.unwrap_or(b.task.created_at);
        start_a.cmp(&start_b).then(a.task.task.cmp(&b.task.task))
    });

    let workflow = api
        .scheduler
        .get_run(&id)
        .await
        .ok()
        .flatten()
        .and_then(|run_record| {
            let deps: Vec<WorkflowTask> = tasks
                .iter()
                .map(|t| WorkflowTask {
                    name: t.task.task.clone(),
                    depends_on: t.deps.clone(),
                    executor: format!("{:?}", t.task.status),
                })
                .collect();
            if deps.is_empty() {
                return None;
            }
            Some(WorkflowInfo {
                name: run_record.workflow,
                tasks: deps,
            })
        });

    Json(RunDetail {
        run,
        tasks,
        workflow,
    })
    .into_response()
}

async fn ui() -> Html<&'static str> {
    Html(include_str!("../ui/index.html"))
}

fn load_and_compile(
    path: &std::path::Path,
) -> Result<(Workflow, ork_core::compiled::CompiledWorkflow), ()> {
    let wf = Workflow::load(path).map_err(|_| ())?;
    let root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let compiled = wf.compile(root).map_err(|_| ())?;
    Ok((wf, compiled))
}
