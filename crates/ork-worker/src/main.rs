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

use axum::{response::IntoResponse, routing::post, Json, Router};
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
