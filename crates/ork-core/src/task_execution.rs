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

    if !task_with_workflow.depends_on.is_empty()
        && let Some(outputs) = outputs_by_run.get(&task_with_workflow.run_id)
    {
        let mut upstream_map = serde_json::Map::new();
        for dep in &task_with_workflow.depends_on {
            let value = outputs.get(dep).cloned().unwrap_or(serde_json::Value::Null);
            upstream_map.insert(dep.clone(), value);
        }

        if !upstream_map.is_empty() {
            let upstream_value = serde_json::Value::Object(upstream_map);
            if let Some(obj) = params.as_object_mut() {
                obj.entry("upstream".to_string())
                    .or_insert(upstream_value.clone());

                match obj.get_mut("task_input") {
                    Some(task_input) => {
                        if task_input.is_null() {
                            *task_input = serde_json::json!({
                                "upstream": upstream_value.clone()
                            });
                        } else if let Some(input_obj) = task_input.as_object_mut()
                            && input_obj.is_empty()
                        {
                            input_obj.insert("upstream".to_string(), upstream_value.clone());
                        }
                    }
                    None => {
                        obj.insert(
                            "task_input".to_string(),
                            serde_json::json!({
                                "upstream": upstream_value.clone()
                            }),
                        );
                    }
                }
            }
        }
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

#[cfg(test)]
mod tests;
