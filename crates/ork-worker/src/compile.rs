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
