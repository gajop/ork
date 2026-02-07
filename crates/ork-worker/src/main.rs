//! Ork Worker - Stateless HTTP server for workflow compilation and task execution
//!
//! The worker container embeds:
//! - workflow.yaml
//! - tasks.py / Rust task code
//! - dependencies
//!
//! It provides two endpoints:
//! - POST /compile - Parse and compile DAG from embedded workflow
//! - POST /execute - Execute a specific task

use axum::{Json, Router, response::IntoResponse, routing::post};
use clap::Parser;
use std::sync::Arc;
use tracing::info;

mod compile;
mod execute;

use compile::compile_handler;
use execute::execute_handler;

#[derive(Parser, Debug)]
#[command(name = "ork-worker")]
#[command(about = "Ork Worker - Stateless task execution server")]
struct Args {
    /// Address to bind to
    #[arg(long, default_value = "127.0.0.1:8081")]
    addr: String,

    /// Path to workflow YAML (embedded in container)
    #[arg(long, default_value = "workflow.yaml")]
    workflow_path: String,

    /// Working directory for task execution
    #[arg(long, default_value = ".")]
    working_dir: String,
}

#[derive(Clone)]
pub struct WorkerState {
    pub workflow_path: String,
    pub working_dir: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting Ork Worker");
    info!("  Address: {}", args.addr);
    info!("  Workflow: {}", args.workflow_path);
    info!("  Working directory: {}", args.working_dir);

    let state = WorkerState {
        workflow_path: args.workflow_path,
        working_dir: args.working_dir,
    };

    // Build router
    let app = Router::new()
        .route("/health", post(health_handler))
        .route("/compile", post(compile_handler))
        .route("/execute", post(execute_handler))
        .with_state(Arc::new(state));

    // Bind and serve
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!("Worker listening on {}", args.addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "ork-worker"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[test]
    fn test_args_defaults() {
        let args = Args::parse_from(["ork-worker"]);
        assert_eq!(args.addr, "127.0.0.1:8081");
        assert_eq!(args.workflow_path, "workflow.yaml");
        assert_eq!(args.working_dir, ".");
    }

    #[tokio::test]
    async fn test_health_handler_payload() {
        let response = health_handler().await.into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "ork-worker");
    }
}
