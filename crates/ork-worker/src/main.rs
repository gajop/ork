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
use std::future::Future;
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

fn build_app(state: WorkerState) -> Router {
    Router::new()
        .route("/health", post(health_handler))
        .route("/compile", post(compile_handler))
        .route("/execute", post(execute_handler))
        .with_state(Arc::new(state))
}

async fn run_with_shutdown<S>(
    listener: tokio::net::TcpListener,
    state: WorkerState,
    shutdown: S,
) -> anyhow::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    let app = build_app(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt::try_init();
}

async fn run_from_args<S>(args: Args, shutdown: S) -> anyhow::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    info!("Starting Ork Worker");
    info!("  Address: {}", args.addr);
    info!("  Workflow: {}", args.workflow_path);
    info!("  Working directory: {}", args.working_dir);

    let state = WorkerState {
        workflow_path: args.workflow_path,
        working_dir: args.working_dir,
    };

    // Bind and serve
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    info!("Worker listening on {}", args.addr);

    run_with_shutdown(listener, state, shutdown).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    run_from_args(args, std::future::pending::<()>()).await
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
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use tokio::time::{Duration, sleep};

    #[test]
    fn test_args_defaults() {
        let args = Args::parse_from(["ork-worker"]);
        assert_eq!(args.addr, "127.0.0.1:8081");
        assert_eq!(args.workflow_path, "workflow.yaml");
        assert_eq!(args.working_dir, ".");
    }

    #[test]
    fn test_args_custom_values() {
        let args = Args::parse_from([
            "ork-worker",
            "--addr",
            "0.0.0.0:9000",
            "--workflow-path",
            "/tmp/workflow.yaml",
            "--working-dir",
            "/tmp",
        ]);
        assert_eq!(args.addr, "0.0.0.0:9000");
        assert_eq!(args.workflow_path, "/tmp/workflow.yaml");
        assert_eq!(args.working_dir, "/tmp");
    }

    fn free_local_addr() -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
        listener.local_addr().expect("local addr")
    }

    #[tokio::test]
    async fn test_health_handler_payload() {
        let response = health_handler().await.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "ork-worker");
    }

    #[tokio::test]
    async fn test_run_with_shutdown_serves_health_route() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let state = WorkerState {
            workflow_path: "workflow.yaml".to_string(),
            working_dir: ".".to_string(),
        };

        let server = tokio::spawn(async move {
            run_with_shutdown(listener, state, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        // Retry briefly until the server starts listening.
        let client = reqwest::Client::new();
        let response = loop {
            match client.post(format!("http://{addr}/health")).send().await {
                Ok(resp) => break resp,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        };
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = response.json().await.expect("health json");
        assert_eq!(body["status"], "healthy");

        let _ = shutdown_tx.send(());
        let result = server.await.expect("join server");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_from_args_serves_health_route() {
        init_tracing();
        let addr = free_local_addr();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let args = Args {
            addr: addr.to_string(),
            workflow_path: "workflow.yaml".to_string(),
            working_dir: ".".to_string(),
        };

        let server = tokio::spawn(async move {
            run_from_args(args, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let client = reqwest::Client::new();
        let response = loop {
            match client.post(format!("http://{addr}/health")).send().await {
                Ok(resp) => break resp,
                Err(_) => sleep(Duration::from_millis(50)).await,
            }
        };
        assert_eq!(response.status(), reqwest::StatusCode::OK);

        let _ = shutdown_tx.send(());
        let result = server.await.expect("join server");
        assert!(result.is_ok());
    }
}
