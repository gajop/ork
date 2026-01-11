use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use indexmap::IndexMap;

use ork_core::compiled::CompiledWorkflow;
use ork_core::types::{TaskRun, TaskStatus, TaskStatusFile};
use ork_state::{ObjectStore, StateStore};

use super::types::LocalTaskState;

pub fn classify_tasks(
    workflow: &CompiledWorkflow,
    states: &[LocalTaskState],
) -> (Vec<usize>, Vec<usize>) {
    let mut ready = Vec::new();
    let mut to_skip = Vec::new();

    for (idx, status) in states.iter().enumerate() {
        if !matches!(status, LocalTaskState::Pending) {
            continue;
        }

        let task = &workflow.tasks[idx];
        let all_success = task
            .depends_on
            .iter()
            .all(|dep| matches!(states.get(*dep), Some(LocalTaskState::Success { .. })));
        if all_success {
            ready.push(idx);
            continue;
        }

        let has_failed_dep = task.depends_on.iter().any(|dep| {
            matches!(
                states.get(*dep),
                Some(LocalTaskState::Failed { .. }) | Some(LocalTaskState::Skipped { .. })
            )
        });
        if has_failed_dep {
            to_skip.push(idx);
        }
    }

    (ready, to_skip)
}

pub fn collect_upstream_outputs(
    task_idx: usize,
    workflow: &CompiledWorkflow,
    outputs: &[Option<serde_json::Value>],
) -> IndexMap<String, serde_json::Value> {
    let mut upstream = IndexMap::new();
    if let Some(task) = workflow.tasks.get(task_idx) {
        for &dep in &task.depends_on {
            if let Some(Some(value)) = outputs.get(dep) {
                let name = workflow.tasks[dep].name.clone();
                upstream.insert(name, value.clone());
            }
        }
    }
    upstream
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_failure(
    task_idx: usize,
    error: String,
    workflow: &CompiledWorkflow,
    states: &mut [LocalTaskState],
    attempts: &mut [u32],
    object_store: &Arc<dyn ObjectStore>,
    run_id: &str,
    state: &Arc<dyn StateStore>,
    started_at: chrono::DateTime<Utc>,
) {
    let attempt = attempts.get(task_idx).copied().unwrap_or(1);
    let retries = workflow.tasks.get(task_idx).map(|t| t.retries).unwrap_or(0);

    if attempt <= retries {
        states[task_idx] = LocalTaskState::Pending;
    } else {
        states[task_idx] = LocalTaskState::Failed {
            attempt,
            error: error.clone(),
        };
        let task_name = &workflow.tasks[task_idx].name;
        let _ = object_store
            .write_status(
                run_id,
                task_name,
                &TaskStatusFile {
                    status: TaskStatus::Failed,
                    started_at: Some(started_at),
                    finished_at: Some(Utc::now()),
                    heartbeat_at: None,
                    error: Some(error.clone()),
                },
            )
            .await;
        let _ = state
            .upsert_task_run(TaskRun {
                run_id: run_id.to_string(),
                task: task_name.clone(),
                status: TaskStatus::Failed,
                attempt,
                max_retries: retries,
                created_at: started_at,
                dispatched_at: Some(started_at),
                started_at: Some(started_at),
                finished_at: Some(Utc::now()),
                error: Some(error),
                output: None,
            })
            .await;
        skip_downstream(task_idx, workflow, states);
    }
}

fn skip_downstream(failed_task: usize, workflow: &CompiledWorkflow, states: &mut [LocalTaskState]) {
    let mut to_visit: Vec<usize> = dependents_of(workflow, failed_task);

    let mut visited = HashSet::new();

    while let Some(task_name) = to_visit.pop() {
        if !visited.insert(task_name.clone()) {
            continue;
        }
        if matches!(states.get(task_name), Some(LocalTaskState::Pending)) {
            states[task_name] = LocalTaskState::Skipped {
                reason: format!("dependency {} failed", workflow.tasks[failed_task].name),
            };
        }

        to_visit.extend(dependents_of(workflow, task_name));
    }
}

fn dependents_of(workflow: &CompiledWorkflow, task_idx: usize) -> Vec<usize> {
    let mut deps = Vec::new();
    for (idx, task) in workflow.tasks.iter().enumerate() {
        if task.depends_on.contains(&task_idx) {
            deps.push(idx);
        }
    }
    deps
}
