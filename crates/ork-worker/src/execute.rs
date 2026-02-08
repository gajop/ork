//! Task execution endpoint
//!
//! POST /execute
//! - Executes a specific task from the workflow
//! - Returns output or deferred job information

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use ork_core::executor::{Executor, StatusUpdate};
use ork_executors::process::ProcessExecutor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{Duration, timeout};
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

    // Set up status channel and wait for terminal updates.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;

    let command = req
        .params
        .as_ref()
        .and_then(|p| p.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or(req.task_name.as_str());

    executor
        .execute(req.task_id, command, req.params.clone())
        .await
        .map_err(|e| format!("Execution failed: {}", e))?;

    timeout(
        Duration::from_secs(30),
        wait_for_terminal_update(req.task_id, &mut rx),
    )
    .await
    .map_err(|_| "Execution timed out waiting for completion".to_string())?
}

async fn wait_for_terminal_update(
    task_id: Uuid,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<StatusUpdate>,
) -> Result<serde_json::Value, String> {
    loop {
        let Some(update) = rx.recv().await else {
            return Err("Execution channel closed before completion".to_string());
        };

        if update.task_id != task_id {
            continue;
        }

        match update.status.as_str() {
            "success" => {
                return update
                    .output
                    .ok_or_else(|| "Execution completed without ORK_OUTPUT JSON".to_string());
            }
            "failed" => {
                return Err(update
                    .error
                    .unwrap_or_else(|| "Execution failed".to_string()));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use std::sync::Once;

    fn init_tracing() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_test_writer()
                .with_max_level(tracing::Level::INFO)
                .try_init();
        });
    }

    async fn run_handler(req: ExecuteRequest) -> serde_json::Value {
        init_tracing();
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
                .contains("without ORK_OUTPUT JSON")
        );
    }

    #[tokio::test]
    async fn test_execute_handler_process_success_json_output() {
        let value = run_handler(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "task".to_string(),
            executor_type: "process".to_string(),
            params: Some(serde_json::json!({
                "command": "printf 'ORK_OUTPUT:{\"ok\":true}'"
            })),
        })
        .await;

        assert_eq!(value["status"], "success");
        assert_eq!(value["output"], serde_json::json!({"ok": true}));
        assert!(value.get("deferred").is_none());
    }

    #[tokio::test]
    async fn test_execute_handler_process_success_with_ork_output_prefix_and_deferred() {
        let value = run_handler(ExecuteRequest {
            task_id: Uuid::new_v4(),
            task_name: "task".to_string(),
            executor_type: "process".to_string(),
            params: Some(serde_json::json!({
                "command": "printf 'ORK_OUTPUT:{\"deferred\":[{\"service_type\":\"custom_http\",\"job_id\":\"job-1\",\"url\":\"http://example.com/status\"}]}'"
            })),
        })
        .await;

        assert_eq!(value["status"], "success");
        assert_eq!(value["output"]["deferred"][0]["service_type"], "custom_http");
        assert_eq!(value["deferred"][0]["job_id"], "job-1");
    }

    #[tokio::test]
    async fn test_wait_for_terminal_update_skips_other_tasks_and_uses_failure_default() {
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(StatusUpdate {
            task_id: Uuid::new_v4(),
            status: "success".to_string(),
            log: None,
            output: Some(serde_json::json!({"wrong": true})),
            error: None,
        })
        .expect("send update");
        tx.send(StatusUpdate {
            task_id,
            status: "failed".to_string(),
            log: None,
            output: None,
            error: None,
        })
        .expect("send update");

        let err = wait_for_terminal_update(task_id, &mut rx)
            .await
            .expect_err("should fail");
        assert!(err.contains("Execution failed"));
    }

    #[tokio::test]
    async fn test_wait_for_terminal_update_closed_channel_and_explicit_error() {
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(StatusUpdate {
            task_id,
            status: "failed".to_string(),
            log: None,
            output: None,
            error: Some("boom".to_string()),
        })
        .expect("send update");
        let err = wait_for_terminal_update(task_id, &mut rx)
            .await
            .expect_err("should fail with explicit error");
        assert!(err.contains("boom"));

        let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel::<StatusUpdate>();
        drop(tx2);
        let err2 = wait_for_terminal_update(Uuid::new_v4(), &mut rx2)
            .await
            .expect_err("closed channel should fail");
        assert!(err2.contains("channel closed"));
    }
}
