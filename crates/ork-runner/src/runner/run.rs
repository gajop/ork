use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::task::JoinHandle;
use tokio::{fs, sync::Semaphore};
use uuid::Uuid;

use ork_core::compiled::CompiledWorkflow;
use ork_core::error::OrkResult;
use ork_core::types::{TaskRun, TaskSpec, TaskStatus, TaskStatusFile};
use ork_state::{ObjectStore, StateStore};

use crate::executor::{TaskContext, TaskExecutionResult, TaskExecutionStatus, execute_task};
use crate::runner::helpers;
use crate::runner::types::{LocalTaskState, RunSummary};

type TaskJoinHandle = JoinHandle<(usize, OrkResult<TaskExecutionResult>, DateTime<Utc>)>;

pub async fn run_workflow(
    workflow: &CompiledWorkflow,
    base_dir: &Path,
    max_parallel: usize,
    run_id: Option<String>,
    state: Arc<dyn StateStore>,
    object_store: Arc<dyn ObjectStore>,
) -> OrkResult<RunSummary> {
    fs::create_dir_all(base_dir)
        .await
        .map_err(|source| ork_core::error::OrkError::WriteFile {
            path: base_dir.to_path_buf(),
            source,
        })?;

    let run_id = run_id.unwrap_or_else(|| format!("{}-{}", workflow.name, Uuid::new_v4()));
    let run_dir = base_dir.join(&run_id);
    fs::create_dir_all(&run_dir)
        .await
        .map_err(|source| ork_core::error::OrkError::WriteFile {
            path: run_dir.clone(),
            source,
        })?;
    let object_store = object_store;
    let _ = state
        .update_run_status(&run_id, ork_core::types::RunStatus::Running)
        .await;

    let task_count = workflow.tasks.len();
    let mut task_states: Vec<LocalTaskState> = vec![LocalTaskState::Pending; task_count];
    let mut attempts: Vec<u32> = vec![0; task_count];
    let mut upstream_outputs: Vec<Option<serde_json::Value>> = vec![None; task_count];

    let mut running: FuturesUnordered<TaskJoinHandle> = FuturesUnordered::new();
    let semaphore = Arc::new(Semaphore::new(max_parallel.max(1)));
    let started_at = Instant::now();

    loop {
        let (ready, to_skip) = helpers::classify_tasks(workflow, &task_states);

        mark_skipped(&mut task_states, to_skip);
        dispatch_ready_tasks(
            ready,
            workflow,
            &mut attempts,
            &mut task_states,
            &upstream_outputs,
            &run_id,
            &object_store,
            &state,
            &semaphore,
            &mut running,
        )
        .await?;

        if running.is_empty() {
            if !has_pending(&task_states) {
                break;
            }
            continue;
        }

        if let Some(joined) = running.next().await {
            match joined {
                Ok((task_idx, result, started_at)) => {
                    process_task_result(
                        task_idx,
                        result,
                        started_at,
                        workflow,
                        &mut upstream_outputs,
                        &mut task_states,
                        &mut attempts,
                        &object_store,
                        &run_id,
                        &state,
                    )
                    .await;
                }
                Err(join_err) => {
                    if !workflow.tasks.is_empty() {
                        helpers::handle_failure(
                            0,
                            join_err.to_string(),
                            workflow,
                            &mut task_states,
                            &mut attempts,
                            &object_store,
                            &run_id,
                            &state,
                            Utc::now(),
                        )
                        .await;
                    }
                }
            }
        }
    }

    let finished_at = Instant::now();
    let final_status = if task_states.iter().any(|state| {
        matches!(
            state,
            LocalTaskState::Failed { .. } | LocalTaskState::Skipped { .. }
        )
    }) {
        ork_core::types::RunStatus::Failed
    } else {
        ork_core::types::RunStatus::Success
    };
    let _ = state.update_run_status(&run_id, final_status).await;

    Ok(RunSummary {
        run_id,
        started_at,
        finished_at,
        statuses: workflow
            .tasks
            .iter()
            .enumerate()
            .map(|(idx, task)| (task.name.clone(), task_states[idx].clone()))
            .collect::<HashMap<_, _>>(),
        run_dir,
    })
}

fn mark_skipped(task_states: &mut [LocalTaskState], to_skip: Vec<usize>) {
    for idx in to_skip {
        task_states[idx] = LocalTaskState::Skipped {
            reason: "dependency failed".into(),
        };
    }
}

fn has_pending(states: &[LocalTaskState]) -> bool {
    states
        .iter()
        .any(|status| matches!(status, LocalTaskState::Pending))
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_ready_tasks(
    ready: Vec<usize>,
    workflow: &CompiledWorkflow,
    attempts: &mut [u32],
    task_states: &mut [LocalTaskState],
    upstream_outputs: &[Option<serde_json::Value>],
    run_id: &str,
    object_store: &Arc<dyn ObjectStore>,
    state: &Arc<dyn StateStore>,
    semaphore: &Arc<Semaphore>,
    running: &mut FuturesUnordered<TaskJoinHandle>,
) -> OrkResult<()> {
    for task_idx in ready {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let task_def = workflow.tasks.get(task_idx).unwrap().clone();
        let task_name = workflow.tasks[task_idx].name.clone();
        let attempt = attempts.get(task_idx).copied().unwrap_or(0) + 1;
        attempts[task_idx] = attempt;

        task_states[task_idx] = LocalTaskState::Running { attempt };
        let workflow_name = workflow.name.clone();
        let run_id_clone = run_id.to_string();
        let object_store = object_store.clone();
        let state_clone = state.clone();
        let upstream = helpers::collect_upstream_outputs(task_idx, workflow, upstream_outputs);
        let started_at = Utc::now();

        let spec = TaskSpec::from_compiled(
            workflow,
            run_id_clone.clone(),
            task_idx,
            attempt,
            upstream.clone(),
        )
        .expect("task exists");
        let spec_path = object_store.write_spec(&spec).await?;
        let output_path = object_store.output_path(&run_id_clone, &task_name);
        let task_dir = object_store.task_dir(&run_id_clone, &task_name);

        running.push(tokio::spawn(async move {
            let status = TaskStatusFile {
                status: TaskStatus::Running,
                started_at: Some(started_at),
                finished_at: None,
                heartbeat_at: Some(Utc::now()),
                error: None,
            };
            let _ = object_store
                .write_status(&run_id_clone, &task_name, &status)
                .await;

            let _ = state_clone
                .upsert_task_run(TaskRun {
                    run_id: run_id_clone.clone(),
                    task: task_name.clone(),
                    status: TaskStatus::Running,
                    attempt,
                    max_retries: task_def.retries,
                    created_at: started_at,
                    dispatched_at: Some(started_at),
                    started_at: Some(started_at),
                    finished_at: None,
                    error: None,
                    output: None,
                })
                .await;

            let ctx = TaskContext {
                workflow_name,
                task_name: task_name.clone(),
                run_id: run_id_clone,
                attempt,
                task_dir,
                spec_path,
                output_path,
            };

            let result = execute_task(&task_def, &upstream, &ctx).await;
            drop(permit);
            (task_idx, result, started_at)
        }));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_task_result(
    task_idx: usize,
    result: OrkResult<TaskExecutionResult>,
    started_at: chrono::DateTime<Utc>,
    workflow: &CompiledWorkflow,
    upstream_outputs: &mut Vec<Option<serde_json::Value>>,
    task_states: &mut Vec<LocalTaskState>,
    attempts: &mut Vec<u32>,
    object_store: &Arc<dyn ObjectStore>,
    run_id: &str,
    state: &Arc<dyn StateStore>,
) {
    match result {
        Ok(TaskExecutionResult {
            status: TaskExecutionStatus::Success,
            output,
            ..
        }) => {
            upstream_outputs[task_idx] = output.clone();
            let attempt = attempts.get(task_idx).copied().unwrap_or(1);
            task_states[task_idx] = LocalTaskState::Success {
                attempt,
                output: output.clone(),
            };

            if let Some(output) = task_states.get(task_idx).and_then(|state| match state {
                LocalTaskState::Success { output, .. } => output.clone(),
                _ => None,
            }) {
                let _ = object_store
                    .write_output(run_id, &workflow.tasks[task_idx].name, &output)
                    .await;
            }

            let _ = object_store
                .write_status(
                    run_id,
                    &workflow.tasks[task_idx].name,
                    &TaskStatusFile {
                        status: TaskStatus::Success,
                        started_at: Some(started_at),
                        finished_at: Some(Utc::now()),
                        heartbeat_at: None,
                        error: None,
                    },
                )
                .await;

            let _ = state
                .upsert_task_run(TaskRun {
                    run_id: run_id.to_string(),
                    task: workflow.tasks[task_idx].name.clone(),
                    status: TaskStatus::Success,
                    attempt,
                    max_retries: workflow.tasks.get(task_idx).map(|t| t.retries).unwrap_or(0),
                    created_at: started_at,
                    dispatched_at: Some(started_at),
                    started_at: Some(started_at),
                    finished_at: Some(Utc::now()),
                    error: None,
                    output,
                })
                .await;
        }
        Ok(TaskExecutionResult {
            status: TaskExecutionStatus::TimedOut,
            ..
        }) => {
            helpers::handle_failure(
                task_idx,
                "task timed out".into(),
                workflow,
                task_states,
                attempts,
                object_store,
                run_id,
                state,
                started_at,
            )
            .await;
        }
        Ok(TaskExecutionResult {
            status: TaskExecutionStatus::Failed(error),
            ..
        }) => {
            helpers::handle_failure(
                task_idx,
                error,
                workflow,
                task_states,
                attempts,
                object_store,
                run_id,
                state,
                started_at,
            )
            .await;
        }
        Err(err) => {
            helpers::handle_failure(
                task_idx,
                err.to_string(),
                workflow,
                task_states,
                attempts,
                object_store,
                run_id,
                state,
                started_at,
            )
            .await;
        }
    }
}
