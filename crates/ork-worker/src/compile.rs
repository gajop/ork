//! Workflow compilation endpoint
//!
//! POST /compile
//! - Loads embedded workflow.yaml
//! - Parses and validates the DAG using core workflow pipeline
//! - Returns compiled workflow structure

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use ork_core::workflow::Workflow;
use serde::{Deserialize, Serialize};
use std::path::Path;
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

    // Parse YAML into core Workflow type
    let workflow: Workflow = serde_yaml::from_str(&workflow_content).map_err(|e| {
        error!("Failed to parse workflow YAML: {}", e);
        (
            StatusCode::BAD_REQUEST,
            format!("Invalid workflow YAML: {}", e),
        )
    })?;

    // Validate workflow
    workflow.validate().map_err(|e| {
        error!("Workflow validation failed: {}", e);
        (StatusCode::BAD_REQUEST, format!("Invalid workflow: {}", e))
    })?;

    // Determine root path for compilation
    let root = Path::new(workflow_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
    let root = root.canonicalize().unwrap_or(root);

    // Compile workflow
    let compiled = workflow.compile(&root).map_err(|e| {
        error!("Workflow compilation failed: {}", e);
        (
            StatusCode::BAD_REQUEST,
            format!("Failed to compile workflow: {}", e),
        )
    })?;

    // Extract task definitions from compiled workflow
    let tasks: Vec<TaskDefinition> = compiled
        .tasks
        .iter()
        .map(|task| TaskDefinition {
            name: task.name.clone(),
            executor: match task.executor {
                ork_core::workflow::ExecutorKind::Process => "process".to_string(),
                ork_core::workflow::ExecutorKind::Python => "python".to_string(),
                ork_core::workflow::ExecutorKind::CloudRun => "cloudrun".to_string(),
                ork_core::workflow::ExecutorKind::Library => "library".to_string(),
            },
            depends_on: task
                .depends_on
                .iter()
                .filter_map(|&idx| compiled.tasks.get(idx).map(|t| t.name.clone()))
                .collect(),
        })
        .collect();

    info!("Successfully compiled workflow with {} tasks", tasks.len());

    // Convert workflow to JSON for response
    let workflow_json = serde_json::to_value(&workflow).map_err(|e| {
        error!("Failed to serialize workflow: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize workflow: {}", e),
        )
    })?;

    Ok(Json(CompileResponse {
        workflow: workflow_json,
        tasks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_compile_handler_valid_workflow_succeeds() {
        let dir = std::env::temp_dir().join(format!("ork-worker-compile-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"
name: test
tasks:
  t1:
    executor: process
    command: "echo hi"
    output_type:
      data: str
  t2:
    executor: python
    module: "tasks"
    depends_on: ["t1"]
    input_type:
      upstream:
        t1:
          data: str
"#,
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

        // Verify task definitions are correct
        assert_eq!(value["tasks"].as_array().unwrap().len(), 2);
        let t1 = &value["tasks"][0];
        let t2 = &value["tasks"][1];

        assert_eq!(t1["name"], "t1");
        assert_eq!(t1["executor"], "process");
        assert_eq!(t1["depends_on"].as_array().unwrap().len(), 0);

        assert_eq!(t2["name"], "t2");
        assert_eq!(t2["executor"], "python");
        assert_eq!(t2["depends_on"], serde_json::json!(["t1"]));

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
        std::fs::write(&workflow_path, r#"name: test"#).expect("write workflow yaml without tasks");

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
            Err(error) => {
                assert_eq!(error.0, StatusCode::BAD_REQUEST);
                assert!(error.1.contains("Invalid workflow"));
            }
        }
    }

    #[tokio::test]
    async fn test_compile_handler_rejects_invalid_dependency() {
        let dir =
            std::env::temp_dir().join(format!("ork-worker-compile-bad-dep-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"
name: test
tasks:
  task1:
    executor: process
    command: "echo hi"
    depends_on: ["nonexistent"]
"#,
        )
        .expect("write workflow yaml with bad dependency");

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
            Ok(_) => panic!("workflow with invalid dependency should return error"),
            Err(error) => {
                assert_eq!(error.0, StatusCode::BAD_REQUEST);
                assert!(error.1.contains("Invalid workflow"));
            }
        }
    }

    #[tokio::test]
    async fn test_compile_handler_rejects_missing_required_field() {
        let dir = std::env::temp_dir().join(format!(
            "ork-worker-compile-missing-field-{}",
            Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"
name: test
tasks:
  task1:
    executor: library
"#,
        )
        .expect("write workflow yaml with missing required field");

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
            Ok(_) => panic!("workflow with missing required field should return error"),
            Err(error) => {
                assert_eq!(error.0, StatusCode::BAD_REQUEST);
                assert!(error.1.contains("Invalid workflow"));
            }
        }
    }

    #[tokio::test]
    async fn test_compile_handler_rejects_cycle() {
        let dir = std::env::temp_dir().join(format!("ork-worker-compile-cycle-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let workflow_path = dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"
name: test
tasks:
  a:
    executor: process
    command: "echo a"
    depends_on: ["b"]
  b:
    executor: process
    command: "echo b"
    depends_on: ["a"]
"#,
        )
        .expect("write workflow yaml with cycle");

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
            Ok(_) => panic!("workflow with cycle should return error"),
            Err(error) => {
                assert_eq!(error.0, StatusCode::BAD_REQUEST);
                assert!(
                    error.1.contains("Failed to compile workflow")
                        || error.1.contains("cycle")
                        || error.1.contains("Cycle")
                );
            }
        }
    }
}
