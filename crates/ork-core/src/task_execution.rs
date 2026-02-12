use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::NewTask;
use crate::executor::StatusUpdate;
use crate::executor_manager::ExecutorManager;
use crate::models::{TaskStatus, TaskWithWorkflow, Workflow, WorkflowTask, json_inner};

pub async fn execute_task<E: ExecutorManager>(
    task_with_workflow: TaskWithWorkflow,
    workflow_map: Arc<HashMap<Uuid, Workflow>>,
    outputs_by_run: Arc<HashMap<Uuid, HashMap<String, serde_json::Value>>>,
    executor_manager: &E,
    status_tx: tokio::sync::mpsc::UnboundedSender<StatusUpdate>,
) -> (Uuid, Result<String>) {
    if !matches!(task_with_workflow.task_status, TaskStatus::Pending) {
        let err = anyhow::anyhow!(
            "refusing to dispatch task {} in non-pending status {}",
            task_with_workflow.task_id,
            task_with_workflow.task_status.as_str()
        );
        return (task_with_workflow.task_id, Err(err));
    }

    let workflow = match workflow_map.get(&task_with_workflow.workflow_id) {
        Some(workflow) => workflow,
        None => {
            let err = anyhow::anyhow!(
                "workflow {} not found for task {}",
                task_with_workflow.workflow_id,
                task_with_workflow.task_id
            );
            return (task_with_workflow.task_id, Err(err));
        }
    };

    let job_name = resolve_job_name(&task_with_workflow);
    let mut params = task_with_workflow
        .params
        .as_ref()
        .map(|p| json_inner(p).clone())
        .unwrap_or_else(|| serde_json::json!({}));
    if !params.is_object() {
        params = serde_json::json!({});
    }
    if let Some(obj) = params.as_object_mut() {
        let attempt_number = task_with_workflow.attempts.saturating_add(1).max(1);
        let env_value = obj
            .entry("env".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !env_value.is_object() {
            *env_value = serde_json::json!({});
        }
        if let Some(env_obj) = env_value.as_object_mut() {
            env_obj.insert("ORK_ATTEMPT".to_string(), serde_json::json!(attempt_number));
        }
    }

    if let Some(obj) = params.as_object_mut()
        && let Err(err) =
            resolve_bindings_for_task(obj, &task_with_workflow, outputs_by_run.as_ref())
    {
        return (task_with_workflow.task_id, Err(err));
    }

    let execution_result = match executor_manager
        .get_executor(&task_with_workflow.executor_type, workflow)
        .await
    {
        Ok(executor) => {
            executor.set_status_channel(status_tx).await;
            executor
                .execute(task_with_workflow.task_id, &job_name, Some(params))
                .await
        }
        Err(e) => Err(e),
    };

    (task_with_workflow.task_id, execution_result)
}

fn resolve_bindings_for_task(
    params_obj: &mut serde_json::Map<String, serde_json::Value>,
    task: &TaskWithWorkflow,
    outputs_by_run: &HashMap<Uuid, HashMap<String, serde_json::Value>>,
) -> Result<()> {
    let Some(bindings_value) = params_obj.get("task_bindings").cloned() else {
        return Ok(());
    };
    let bindings_obj = bindings_value.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "task_bindings must be an object for task '{}'",
            task.task_name
        )
    })?;

    let run_outputs = outputs_by_run.get(&task.run_id);
    let mut resolved_input = serde_json::Map::new();

    for (arg, binding) in bindings_obj {
        let binding_obj = binding.as_object().ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' binding '{}' must be an object with one of: const, ref",
                task.task_name,
                arg
            )
        })?;
        let has_const = binding_obj.contains_key("const");
        let has_ref = binding_obj.contains_key("ref");
        match (has_const, has_ref) {
            (true, false) => {
                resolved_input.insert(
                    arg.clone(),
                    binding_obj
                        .get("const")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            (false, true) => {
                let ref_path =
                    binding_obj
                        .get("ref")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "task '{}' binding '{}.ref' must be a string path",
                                task.task_name,
                                arg
                            )
                        })?;
                let value = resolve_ref_path(task, ref_path, run_outputs)?;
                resolved_input.insert(arg.clone(), value);
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "task '{}' binding '{}' must define exactly one of: const, ref",
                    task.task_name,
                    arg
                ));
            }
        }
    }

    params_obj.insert(
        "task_input".to_string(),
        serde_json::Value::Object(resolved_input),
    );
    Ok(())
}

fn resolve_ref_path(
    task: &TaskWithWorkflow,
    reference: &str,
    run_outputs: Option<&HashMap<String, serde_json::Value>>,
) -> Result<serde_json::Value> {
    let parts: Vec<&str> = reference.split('.').collect();
    if parts.len() < 3 || parts[0] != "tasks" || parts[2] != "output" {
        return Err(anyhow::anyhow!(
            "task '{}' has unsupported ref '{}': expected tasks.<task>.output[.<field>...]",
            task.task_name,
            reference
        ));
    }
    let dep_task = parts[1];
    if !task.depends_on.iter().any(|dep| dep == dep_task) {
        return Err(anyhow::anyhow!(
            "task '{}' ref '{}' targets task '{}' which is not listed in depends_on",
            task.task_name,
            reference,
            dep_task
        ));
    }

    let outputs = run_outputs.ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: run outputs are unavailable",
            task.task_name,
            reference
        )
    })?;
    let mut value = outputs.get(dep_task).cloned().ok_or_else(|| {
        anyhow::anyhow!(
            "task '{}' ref '{}' cannot be resolved: no output for dependency '{}'",
            task.task_name,
            reference,
            dep_task
        )
    })?;

    for segment in parts.iter().skip(3) {
        let obj = value.as_object().ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' ref '{}' cannot access field '{}' on non-object value",
                task.task_name,
                reference,
                segment
            )
        })?;
        value = obj.get(*segment).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "task '{}' ref '{}' missing field '{}'",
                task.task_name,
                reference,
                segment
            )
        })?;
    }

    Ok(value)
}

fn resolve_job_name(task: &TaskWithWorkflow) -> String {
    let params = task.params.as_ref().map(json_inner);
    let override_name = params
        .and_then(|p| p.get("job_name").and_then(|v| v.as_str()))
        .or_else(|| params.and_then(|p| p.get("command").and_then(|v| v.as_str())))
        .or_else(|| params.and_then(|p| p.get("script").and_then(|v| v.as_str())));

    override_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| task.job_name.clone())
}

pub fn retry_backoff_seconds(attempt: i32) -> u64 {
    if attempt <= 1 {
        return 1;
    }
    let capped = attempt.clamp(1, 10) as u32;
    let backoff = 2_u64.saturating_pow(capped - 1);
    backoff.min(60)
}

pub fn build_run_tasks(
    run_id: Uuid,
    workflow: &Workflow,
    workflow_tasks: &[WorkflowTask],
) -> Vec<NewTask> {
    workflow_tasks
        .iter()
        .map(|task| {
            let mut params = task
                .params
                .as_ref()
                .map(|p| json_inner(p).clone())
                .unwrap_or_else(|| serde_json::json!({}));
            if !params.is_object() {
                params = serde_json::json!({});
            }
            let obj = params.as_object_mut().expect("params object");
            let max_retries = obj
                .get("max_retries")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i64::from(i32::MAX)) as i32;
            let timeout_seconds = obj
                .get("timeout_seconds")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(0, i64::from(i32::MAX)) as i32);
            obj.entry("task_index".to_string())
                .or_insert_with(|| serde_json::json!(task.task_index));
            obj.entry("task_name".to_string())
                .or_insert_with(|| serde_json::json!(task.task_name.clone()));
            obj.entry("workflow_name".to_string())
                .or_insert_with(|| serde_json::json!(workflow.name.clone()));
            obj.entry("run_id".to_string())
                .or_insert_with(|| serde_json::json!(run_id.to_string()));

            NewTask {
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params,
                max_retries,
                timeout_seconds,
            }
        })
        .collect()
}

pub fn build_run_tasks_from_snapshot(
    run_id: Uuid,
    workflow: &Workflow,
    snapshot_tasks: &[crate::database::NewWorkflowTask],
) -> Vec<NewTask> {
    snapshot_tasks
        .iter()
        .map(|task| {
            let mut params = task.params.clone();
            if !params.is_object() {
                params = serde_json::json!({});
            }
            let obj = params.as_object_mut().expect("params object");
            let max_retries = obj
                .get("max_retries")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, i64::from(i32::MAX)) as i32;
            let timeout_seconds = obj
                .get("timeout_seconds")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(0, i64::from(i32::MAX)) as i32);
            obj.entry("task_index".to_string())
                .or_insert_with(|| serde_json::json!(task.task_index));
            obj.entry("task_name".to_string())
                .or_insert_with(|| serde_json::json!(task.task_name.clone()));
            obj.entry("workflow_name".to_string())
                .or_insert_with(|| serde_json::json!(workflow.name.clone()));
            obj.entry("run_id".to_string())
                .or_insert_with(|| serde_json::json!(run_id.to_string()));

            NewTask {
                task_index: task.task_index,
                task_name: task.task_name.clone(),
                executor_type: task.executor_type.clone(),
                depends_on: task.depends_on.clone(),
                params,
                max_retries,
                timeout_seconds,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
