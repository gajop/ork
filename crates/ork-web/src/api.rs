use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, routing::get};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use ork_core::database::Database;

use crate::handlers;

pub use crate::api_helpers::{fmt_time, is_not_found};
use crate::api_realtime::{ui, websocket_handler};
use crate::api_routes::{
    cancel_run, list_runs, list_workflows, pause_run, pause_task, resume_run, resume_task,
    start_run, update_workflow_schedule, workflow_detail,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StateUpdate {
    RunUpdated {
        run_id: String,
        status: String,
    },
    TaskUpdated {
        run_id: String,
        task_id: String,
        status: String,
    },
}

#[derive(Clone)]
pub struct ApiServer {
    pub db: Arc<dyn Database>,
    pub broadcast_tx: broadcast::Sender<StateUpdate>,
}

impl ApiServer {
    pub fn new(db: Arc<dyn Database>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        Self { db, broadcast_tx }
    }

    pub async fn serve(self, addr: SocketAddr) -> JoinHandle<()> {
        let router = build_router(self);

        tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .expect("bind address");
            axum::serve(listener, router).await.expect("server error");
        })
    }
}

pub fn build_router(api: ApiServer) -> Router {
    let cors = tower_http::cors::CorsLayer::very_permissive();
    Router::new()
        .route("/", get(ui))
        .route("/ws", get(websocket_handler))
        .route(
            "/api/runs",
            get(list_runs).post(axum::routing::post(start_run)),
        )
        .route("/api/runs/{id}", get(handlers::run_detail))
        .route("/api/runs/{id}/cancel", axum::routing::post(cancel_run))
        .route("/api/runs/{id}/pause", axum::routing::post(pause_run))
        .route("/api/runs/{id}/resume", axum::routing::post(resume_run))
        .route("/api/tasks/{id}/pause", axum::routing::post(pause_task))
        .route("/api/tasks/{id}/resume", axum::routing::post(resume_task))
        .route(
            "/api/workflows",
            get(list_workflows).post(handlers::create_workflow),
        )
        .route("/api/workflows/{name}", get(workflow_detail))
        .route(
            "/api/workflows/{name}/schedule",
            axum::routing::patch(update_workflow_schedule),
        )
        .with_state(api)
        .layer(cors)
}
