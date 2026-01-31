use anyhow::Result;
use futures::stream::{self, StreamExt};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{error, info};
use uuid::Uuid;

use crate::config::OrchestratorConfig;

use crate::database::{Database, NewTask};

use crate::executor::StatusUpdate;

use crate::executor_manager::ExecutorManager;

use crate::models::{TaskStatus, Workflow, WorkflowTask, json_inner};

#[derive(Debug, Default, Serialize)]
pub struct SchedulerMetrics {
    #[allow(dead_code)]
    pub timestamp: u64,
    pub process_pending_runs_ms: u128,
    pub process_pending_tasks_ms: u128,
    pub process_status_updates_ms: u128,
    pub sleep_ms: u128,
    pub total_loop_ms: u128,
}

pub struct Scheduler<D: Database + 'static, E: ExecutorManager + 'static> {
    db: Arc<D>,
    config: OrchestratorConfig,
    executor_manager: Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
    status_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<StatusUpdate>>>,
}

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub fn new(db: Arc<D>, executor_manager: Arc<E>) -> Self {
        Self::new_with_config(db, executor_manager, OrchestratorConfig::default())
    }

    pub fn new_with_config(
        db: Arc<D>,
        executor_manager: Arc<E>,
        config: OrchestratorConfig,
    ) -> Self {
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        Self {
            db,
            config,
            executor_manager,
            status_tx,
            status_rx: Arc::new(tokio::sync::Mutex::new(status_rx)),
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!(
            "Starting scheduler loop (poll_interval={}s, max_batch={}, max_concurrent_dispatch={}, max_concurrent_status={})",
            self.config.poll_interval_secs,
            self.config.max_tasks_per_batch,
            self.config.max_concurrent_dispatches,
            self.config.max_concurrent_status_checks
        );

        let mut status_rx = self.status_rx.lock().await;
        let mut poll_interval = interval(Duration::from_secs_f64(self.config.poll_interval_secs));
        poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let loop_start = Instant::now();
            let mut metrics = SchedulerMetrics::default();

            // Process pending runs and tasks
            let start = Instant::now();
            let runs_processed = match self.process_pending_runs().await {
                Ok(count) => count,
                Err(e) => {
                    error!("Error processing pending runs: {}", e);
                    0
                }
            };
            metrics.process_pending_runs_ms = start.elapsed().as_millis();

            let start = Instant::now();
            let tasks_processed = match self.process_pending_tasks().await {
                Ok(count) => count,
                Err(e) => {
                    error!("Error processing pending tasks: {}", e);
                    0
                }
            };
            metrics.process_pending_tasks_ms = start.elapsed().as_millis();

            // Process any immediately available status updates
            let start = Instant::now();
            let mut status_updates = Vec::new();
            while let Ok(update) = status_rx.try_recv() {
                status_updates.push(update);
            }
            if !status_updates.is_empty() {
                if let Err(e) = self.process_status_updates(status_updates).await {
                    error!("Error processing status updates: {}", e);
                }
            }
            metrics.process_status_updates_ms = start.elapsed().as_millis();

            // Wait for either a status update or the next poll interval
            let had_work = runs_processed > 0 || tasks_processed > 0;
            let start = Instant::now();

            if !had_work {
                // No work to do - wait for status update or timeout
                tokio::select! {
                    _ = poll_interval.tick() => {
                        // Timeout - continue to next iteration
                    }
                    Some(update) = status_rx.recv() => {
                        // Got status update - process it immediately
                        if let Err(e) = self.process_status_updates(vec![update]).await {
                            error!("Error processing status update: {}", e);
                        }
                    }
                }
            }

            metrics.sleep_ms = start.elapsed().as_millis();
            metrics.total_loop_ms = loop_start.elapsed().as_millis();
            metrics.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Output structured JSON metrics for easy parsing
            let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();
            info!("SCHEDULER_METRICS: {}", metrics_json);
        }
    }

    async fn process_pending_runs(&self) -> Result<usize> {
        let runs = self.db.get_pending_runs().await?;

        if runs.is_empty() {
            return Ok(0);
        }

        let count = runs.len();

        // Get all unique workflow IDs
        let workflow_ids: Vec<Uuid> = runs
            .iter()
            .map(|r| r.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Batch fetch all workflows in a single query
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;

        // Create workflow lookup map
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();

        let mut workflow_tasks_map: HashMap<Uuid, Vec<WorkflowTask>> = HashMap::new();
        for workflow_id in workflow_map.keys() {
            let tasks = self.db.list_workflow_tasks(*workflow_id).await?;
            if !tasks.is_empty() {
                workflow_tasks_map.insert(*workflow_id, tasks);
            }
        }

        // Create tasks first, then update runs to running
        // This ensures runs are only marked as running if tasks were successfully created
        for run in &runs {
            if let Some(workflow) = workflow_map.get(&run.workflow_id) {
                let create_result =
                    if let Some(workflow_tasks) = workflow_tasks_map.get(&run.workflow_id) {
                        let tasks = self.build_run_tasks(run.id, workflow, workflow_tasks);
                        self.db.batch_create_dag_tasks(run.id, &tasks).await
                    } else if workflow.executor_type == "dag" {
                        Err(anyhow::anyhow!(
                            "workflow '{}' is missing compiled workflow_tasks",
                            workflow.name
                        ))
                    } else {
                        let task_count = workflow
                            .task_params
                            .as_ref()
                            .and_then(|p| json_inner(p).get("task_count"))
                            .and_then(|v: &serde_json::Value| v.as_i64())
                            .unwrap_or(3) as i32;
                        self.db
                            .batch_create_tasks(
                                run.id,
                                task_count,
                                &workflow.name,
                                &workflow.executor_type,
                            )
                            .await
                    };

                // Create tasks
                if let Err(e) = create_result {
                    error!("Failed to create tasks for run {}: {}", run.id, e);
                    // Mark run as failed
                    let _ = self
                        .db
                        .update_run_status(run.id, "failed", Some(&e.to_string()))
                        .await;
                    continue;
                }

                // Tasks created successfully - update run to running
                if let Err(e) = self.db.update_run_status(run.id, "running", None).await {
                    error!("Failed to update run {} to running: {}", run.id, e);
                }
            }
        }

        Ok(count)
    }

    async fn process_pending_tasks(&self) -> Result<usize> {
        // OPTIMIZATION: Single JOIN query instead of N+1 queries
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(0);
        }

        let count = tasks.len();

        let workflow_ids: Vec<Uuid> = tasks
            .iter()
            .map(|task| task.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();
        let workflow_map = Arc::new(workflow_map);

        // OPTIMIZATION: Process tasks concurrently with limit to avoid memory explosion
        let results = stream::iter(tasks)
            .map(|task_with_workflow| {
                let executor_manager = &self.executor_manager;
                let status_tx = self.status_tx.clone();
                let workflow_map = workflow_map.clone();

                async move {
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
                    let job_name = resolve_job_name(&task_with_workflow, workflow);
                    let execution_result = match executor_manager
                        .get_executor(&task_with_workflow.executor_type, &workflow)
                        .await
                    {
                        Ok(executor) => {
                            executor.set_status_channel(status_tx).await;
                            executor
                                .execute(
                                    task_with_workflow.task_id,
                                    &job_name,
                                    task_with_workflow
                                        .params
                                        .as_ref()
                                        .map(|p| json_inner(p).clone()),
                                )
                                .await
                        }
                        Err(e) => Err(e),
                    };

                    (task_with_workflow.task_id, execution_result)
                }
            })
            .buffer_unordered(self.config.max_concurrent_dispatches)
            .collect::<Vec<_>>()
            .await;

        // Batch update statuses
        let updates: Vec<(Uuid, &str, Option<&str>, Option<String>)> = results
            .iter()
            .map(|(task_id, result)| match result {
                Ok(execution_name) => (*task_id, "dispatched", Some(execution_name.as_str()), None),
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task_id, e);
                    (*task_id, "failed", None, Some(e.to_string()))
                }
            })
            .collect();

        let updates_ref: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = updates
            .iter()
            .map(|(id, status, exec, err)| (*id, *status, *exec, err.as_deref()))
            .collect();

        let db_update_start = Instant::now();
        self.db.batch_update_task_status(&updates_ref).await?;
        let db_update_ms = db_update_start.elapsed().as_millis();
        info!(
            "Batch updated {} tasks in {}ms",
            updates_ref.len(),
            db_update_ms
        );

        Ok(count)
    }

    async fn process_status_updates(&self, updates: Vec<StatusUpdate>) -> Result<()> {
        let mut task_updates: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = Vec::new();
        let mut runs_to_check = std::collections::HashSet::new();
        let mut failed_by_run: HashMap<Uuid, Vec<String>> = HashMap::new();

        for update in &updates {
            if let Some(log) = update.log.as_ref() {
                if let Err(e) = self.db.append_task_log(update.task_id, log).await {
                    error!("Failed to append log for task {}: {}", update.task_id, e);
                }
            }

            let new_status = match update.status.as_str() {
                "success" => TaskStatus::Success,
                "failed" => TaskStatus::Failed,
                "running" => TaskStatus::Running,
                _ => continue,
            };

            task_updates.push((update.task_id, new_status.as_str(), None, None));

            if matches!(new_status, TaskStatus::Success | TaskStatus::Failed) {
                let (run_id, task_name) = self.db.get_task_identity(update.task_id).await?;
                runs_to_check.insert(run_id);
                if matches!(new_status, TaskStatus::Failed) {
                    failed_by_run.entry(run_id).or_default().push(task_name);
                }
            }
        }

        if !task_updates.is_empty() {
            self.db.batch_update_task_status(&task_updates).await?;
            info!(
                "Processed {} status updates from executors",
                task_updates.len()
            );
        }

        // Propagate dependency failures to downstream tasks
        for (run_id, names) in failed_by_run {
            let mut pending = names;
            let mut seen = std::collections::HashSet::new();
            while !pending.is_empty() {
                let next = self
                    .db
                    .mark_tasks_failed_by_dependency(run_id, &pending, "dependency failed")
                    .await?;
                let mut new_pending = Vec::new();
                for name in next {
                    if seen.insert(name.clone()) {
                        new_pending.push(name);
                    }
                }
                pending = new_pending;
            }
            runs_to_check.insert(run_id);
        }

        for run_id in runs_to_check {
            self.check_run_completion(run_id).await?;
        }

        Ok(())
    }

    fn build_run_tasks(
        &self,
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
                }
            })
            .collect()
    }

    async fn check_run_completion(&self, run_id: Uuid) -> Result<()> {
        // Use a COUNT query instead of fetching all tasks
        let (total, completed, failed) = self.db.get_run_task_stats(run_id).await?;

        if completed == total {
            let new_status = if failed > 0 {
                TaskStatus::Failed
            } else {
                TaskStatus::Success
            };

            self.db
                .update_run_status(run_id, new_status.as_str(), None)
                .await?;
            info!(
                "Run {} completed with status: {} ({}/{} tasks)",
                run_id,
                new_status.as_str(),
                completed,
                total
            );
        }

        Ok(())
    }
}

fn resolve_job_name(task: &crate::models::TaskWithWorkflow, workflow: &Workflow) -> String {
    let params = task.params.as_ref().map(json_inner);
    let override_name = params
        .and_then(|p| p.get("job_name").and_then(|v| v.as_str()))
        .or_else(|| params.and_then(|p| p.get("command").and_then(|v| v.as_str())))
        .or_else(|| params.and_then(|p| p.get("script").and_then(|v| v.as_str())));

    override_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| workflow.job_name.clone())
}
