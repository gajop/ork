//! Workflow compilation endpoint
//!
//! POST /compile
//! - Loads embedded workflow.yaml
//! - Parses and validates the DAG
//! - Returns compiled workflow structure

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::WorkerState;

#[derive(Debug, Deserialize)]
pub struct CompileRequest {
    /// Optional: override workflow path
    pub workflow_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompileResponse {
    pub workflow: serde_json::Value,
    pub tasks: Vec<TaskDefinition>,
}

#[derive(Debug, Serialize)]
pub struct TaskDefinition {
    pub name: String,
    pub executor: String,
    pub depends_on: Vec<String>,
}

pub async fn compile_handler(
    State(state): State<Arc<WorkerState>>,
    Json(req): Json<CompileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let workflow_path = req.workflow_path.as_ref().unwrap_or(&state.workflow_path);

    info!("Compiling workflow from: {}", workflow_path);

    // Read workflow file
    let workflow_content = tokio::fs::read_to_string(workflow_path)
        .await
        .map_err(|e| {
            error!("Failed to read workflow file: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read workflow: {}", e),
            )
        })?;

    // Parse YAML
    let workflow: serde_json::Value = serde_yaml::from_str(&workflow_content).map_err(|e| {
        error!("Failed to parse workflow YAML: {}", e);
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid workflow YAML: {}", e),
        )
    })?;

    // Extract task definitions
    let tasks = extract_task_definitions(&workflow).map_err(|e| {
        error!("Failed to extract task definitions: {}", e);
        (StatusCode::BAD_REQUEST, e)
    })?;

    info!("Successfully compiled workflow with {} tasks", tasks.len());

    Ok(Json(CompileResponse { workflow, tasks }))
}

fn extract_task_definitions(workflow: &serde_json::Value) -> Result<Vec<TaskDefinition>, String> {
    let tasks_obj = workflow
        .get("tasks")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Missing 'tasks' field in workflow".to_string())?;

    let mut tasks = Vec::new();

    for (name, task_def) in tasks_obj {
        let executor = task_def
            .get("executor")
            .and_then(|v| v.as_str())
            .unwrap_or("process")
            .to_string();

        let depends_on = task_def
            .get("depends_on")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        tasks.push(TaskDefinition {
            name: name.clone(),
            executor,
            depends_on,
        });
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use uuid::Uuid;

    #[test]
    fn test_extract_task_definitions_success() {
        let workflow = serde_json::json!({
            "name": "wf",
            "tasks": {
                "a": { "executor": "process" },
                "b": { "executor": "python", "depends_on": ["a"] }
            }
        });

        let tasks = extract_task_definitions(&workflow).expect("task extraction should succeed");
        assert_eq!(tasks.len(), 2);
        assert!(
            tasks
                .iter()
                .any(|t| t.name == "a" && t.executor == "process")
        );
        assert!(
            tasks
                .iter()
                .any(|t| t.name == "b" && t.depends_on == vec!["a".to_string()])
        );
    }

    #[test]
    fn test_extract_task_definitions_missing_tasks_field() {
        let workflow = serde_json::json!({ "name": "wf" });
        let err = extract_task_definitions(&workflow).expect_err("missing tasks should error");
        assert!(err.contains("Missing 'tasks' field"));
    }

    #[tokio::test]
    async fn test_compile_handler_success() {
        let dir = std::env::temp_dir().join(format!("ork-worker-compile-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"{"name":"test","tasks":{"t1":{"executor":"process","command":"echo hi"}}}"#,
        )
        .expect("write workflow yaml");

        let state = std::sync::Arc::new(crate::WorkerState {
            workflow_path: workflow_path.to_string_lossy().to_string(),
            working_dir: ".".to_string(),
        });
        let response = compile_handler(
            State(state),
            Json(CompileRequest {
                workflow_path: None,
            }),
        )
        .await
        .expect("compile handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let value: serde_json::Value =
            serde_json::from_slice(&body).expect("response should be valid json");
        assert_eq!(value["tasks"][0]["name"], "t1");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_compile_handler_missing_file_returns_error() {
        let missing_path = format!("/tmp/does-not-exist-{}.yaml", Uuid::new_v4());
        let state = std::sync::Arc::new(crate::WorkerState {
            workflow_path: missing_path.clone(),
            working_dir: ".".to_string(),
        });
        let result = compile_handler(
            State(state),
            Json(CompileRequest {
                workflow_path: None,
            }),
        )
        .await;
        match result {
            Ok(_) => panic!("missing workflow should return error"),
            Err(error) => assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR),
        }
    }

    #[tokio::test]
    async fn test_compile_handler_invalid_yaml_returns_bad_request() {
        let dir =
            std::env::temp_dir().join(format!("ork-worker-compile-invalid-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(&workflow_path, "name: [unterminated").expect("write invalid workflow yaml");

        let state = std::sync::Arc::new(crate::WorkerState {
            workflow_path: workflow_path.to_string_lossy().to_string(),
            working_dir: ".".to_string(),
        });
        let result = compile_handler(
            State(state),
            Json(CompileRequest {
                workflow_path: None,
            }),
        )
        .await;

        let _ = std::fs::remove_dir_all(dir);
        match result {
            Ok(_) => panic!("invalid yaml should return error"),
            Err(error) => assert_eq!(error.0, StatusCode::BAD_REQUEST),
        }
    }

    #[tokio::test]
    async fn test_compile_handler_missing_tasks_returns_bad_request() {
        let dir = std::env::temp_dir().join(format!(
            "ork-worker-compile-missing-tasks-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(&workflow_path, r#"{"name":"test"}"#)
            .expect("write workflow yaml without tasks");

        let state = std::sync::Arc::new(crate::WorkerState {
            workflow_path: workflow_path.to_string_lossy().to_string(),
            working_dir: ".".to_string(),
        });
        let result = compile_handler(
            State(state),
            Json(CompileRequest {
                workflow_path: None,
            }),
        )
        .await;

        let _ = std::fs::remove_dir_all(dir);
        match result {
            Ok(_) => panic!("workflow without tasks should return error"),
            Err(error) => assert_eq!(error.0, StatusCode::BAD_REQUEST),
        }
    }
}
