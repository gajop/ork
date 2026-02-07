//! Task execution endpoint
//!
//! POST /execute
//! - Executes a specific task from the workflow
//! - Returns output or deferred job information

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use ork_core::executor::Executor;
use ork_executors::process::ProcessExecutor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

use crate::WorkerState;

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    pub task_id: Uuid,
    pub task_name: String,
    pub executor_type: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum ExecuteResponse {
    #[serde(rename = "success")]
    Success {
        output: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        deferred: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename = "failed")]
    Failed { error: String },
}

pub async fn execute_handler(
    State(state): State<Arc<WorkerState>>,
    Json(req): Json<ExecuteRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    info!(
        "Executing task: {} (type: {})",
        req.task_name, req.executor_type
    );

    // Execute based on executor type
    let result = match req.executor_type.as_str() {
        "process" | "python" => execute_process_task(&state, &req).await,
        "library" => {
            // Library executor would require loading dynamic libraries
            Err("Library executor not yet supported in worker".to_string())
        }
        _ => Err(format!("Unsupported executor type: {}", req.executor_type)),
    };

    match result {
        Ok(output) => {
            // Check if output contains deferrables
            let deferred = output.get("deferred").and_then(|v| v.as_array()).cloned();

            if deferred.is_some() {
                info!("Task {} returned deferrables", req.task_name);
            }

            Ok(Json(ExecuteResponse::Success { output, deferred }))
        }
        Err(e) => {
            error!("Task {} failed: {}", req.task_name, e);
            Ok(Json(ExecuteResponse::Failed { error: e }))
        }
    }
}

async fn execute_process_task(
    state: &WorkerState,
    req: &ExecuteRequest,
) -> Result<serde_json::Value, String> {
    // Create process executor
    let executor = ProcessExecutor::new(Some(state.working_dir.clone()));

    // Set up a dummy status channel (worker doesn't need it)
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;

    // Execute the task
    let result = executor
        .execute(req.task_id, &req.task_name, req.params.clone())
        .await
        .map_err(|e| format!("Execution failed: {}", e))?;

    // Parse the result as JSON
    // Process executor returns output with "ORK_OUTPUT:" prefix
    let output_str = if result.starts_with("ORK_OUTPUT:") {
        result.trim_start_matches("ORK_OUTPUT:")
    } else {
        &result
    };

    let output: serde_json::Value = serde_json::from_str(output_str)
        .map_err(|e| format!("Failed to parse task output as JSON: {}", e))?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn run_handler(req: ExecuteRequest) -> serde_json::Value {
        let state = std::sync::Arc::new(crate::WorkerState {
            workflow_path: "workflow.yaml".to_string(),
            working_dir: ".".to_string(),
        });
        let response = execute_handler(State(state), Json(req))
            .await
            .expect("handler should return Ok response")
            .into_response();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&body).expect("response should be valid json")
    }

    #[tokio::test]
    async fn test_execute_handler_rejects_unsupported_executor() {
        let value = run_handler(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "task".to_string(),
            executor_type: "unknown".to_string(),
            params: None,
        })
        .await;

        assert_eq!(value["status"], "failed");
        assert!(
            value["error"]
                .as_str()
                .expect("error should be string")
                .contains("Unsupported executor type")
        );
    }

    #[tokio::test]
    async fn test_execute_handler_rejects_library_executor() {
        let value = run_handler(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "task".to_string(),
            executor_type: "library".to_string(),
            params: None,
        })
        .await;

        assert_eq!(value["status"], "failed");
        assert!(
            value["error"]
                .as_str()
                .expect("error should be string")
                .contains("not yet supported")
        );
    }

    #[tokio::test]
    async fn test_execute_handler_process_returns_failed_for_non_json_output() {
        let value = run_handler(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "task".to_string(),
            executor_type: "process".to_string(),
            params: Some(serde_json::json!({
                "command": "echo hello"
            })),
        })
        .await;

        assert_eq!(value["status"], "failed");
        assert!(
            value["error"]
                .as_str()
                .expect("error should be string")
                .contains("Failed to parse task output as JSON")
        );
    }
}
