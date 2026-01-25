use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::config::OrchestratorConfig;
use crate::db::Database;
use crate::executors::ExecutorManager;
use crate::models::{ExecutionStatus, TaskStatus, Workflow};

#[derive(Debug, Default)]
pub struct SchedulerMetrics {
    pub process_pending_runs_ms: u128,
    pub process_pending_tasks_ms: u128,
    pub check_running_tasks_ms: u128,
    pub sleep_ms: u128,
    pub total_loop_ms: u128,
}

pub struct Scheduler {
    db: Arc<Database>,
    config: OrchestratorConfig,
    executor_manager: ExecutorManager,
}

impl Scheduler {
    pub fn new(db: Arc<Database>) -> Self {
        Self::new_with_config(db, OrchestratorConfig::default())
    }

    pub fn new_with_config(db: Arc<Database>, config: OrchestratorConfig) -> Self {
        Self {
            db,
            config,
            executor_manager: ExecutorManager::new(),
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

        loop {
            let loop_start = Instant::now();
            let mut metrics = SchedulerMetrics::default();

            let start = Instant::now();
            if let Err(e) = self.process_pending_runs().await {
                error!("Error processing pending runs: {}", e);
            }
            metrics.process_pending_runs_ms = start.elapsed().as_millis();

            let start = Instant::now();
            if let Err(e) = self.process_pending_tasks().await {
                error!("Error processing pending tasks: {}", e);
            }
            metrics.process_pending_tasks_ms = start.elapsed().as_millis();

            let start = Instant::now();
            if let Err(e) = self.check_running_tasks().await {
                error!("Error checking running tasks: {}", e);
            }
            metrics.check_running_tasks_ms = start.elapsed().as_millis();

            let start = Instant::now();
            sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
            metrics.sleep_ms = start.elapsed().as_millis();

            metrics.total_loop_ms = loop_start.elapsed().as_millis();

            info!(
                "Scheduler loop: {}ms total (runs:{}ms tasks:{}ms status:{}ms sleep:{}ms)",
                metrics.total_loop_ms,
                metrics.process_pending_runs_ms,
                metrics.process_pending_tasks_ms,
                metrics.check_running_tasks_ms,
                metrics.sleep_ms
            );
        }
    }

    async fn process_pending_runs(&self) -> Result<()> {
        let runs = self.db.get_pending_runs().await?;

        for run in runs {
            info!("Processing pending run: {}", run.id);

            // Get the workflow
            let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = $1")
                .bind(run.workflow_id)
                .fetch_one(self.db.pool())
                .await?;

            // Register workflow executor if not already done
            self.executor_manager.register_workflow(&workflow).await?;

            // Update run to running
            self.db.update_run_status(run.id, "running", None).await?;

            // Generate tasks based on workflow configuration
            // For now, we'll generate a simple sequence of tasks
            let task_count = workflow
                .task_params
                .as_ref()
                .and_then(|p| p.0.get("task_count"))
                .and_then(|v| v.as_i64())
                .unwrap_or(3) as i32;

            for i in 0..task_count {
                let params = serde_json::json!({
                    "task_index": i,
                    "run_id": run.id.to_string(),
                    "workflow_name": workflow.name,
                });

                self.db.create_task(run.id, i, Some(params)).await?;
                info!("Created task {}/{} for run {}", i + 1, task_count, run.id);
            }

            info!("Created {} tasks for run {}", task_count, run.id);
        }

        Ok(())
    }

    async fn process_pending_tasks(&self) -> Result<()> {
        let query_start = Instant::now();
        // OPTIMIZATION: Single JOIN query instead of N+1 queries
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(());
        }

        let query_ms = query_start.elapsed().as_millis();
        info!("Processing {} pending tasks (query: {}ms)", tasks.len(), query_ms);

        // Ensure all workflows have registered executors
        let mut workflow_ids = std::collections::HashSet::new();
        for task in &tasks {
            workflow_ids.insert(task.workflow_id);
        }

        for workflow_id in workflow_ids {
            // Check if executor already registered, if not register it
            if self.executor_manager.get_executor(workflow_id).await.is_err() {
                let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = $1")
                    .bind(workflow_id)
                    .fetch_one(self.db.pool())
                    .await?;
                self.executor_manager.register_workflow(&workflow).await?;
            }
        }

        // OPTIMIZATION: Process tasks concurrently with limit to avoid memory explosion
        let dispatch_start = Instant::now();
        let results = stream::iter(tasks)
            .map(|task_with_workflow| {
                let executor_manager = &self.executor_manager;

                async move {
                    let execution_result = match executor_manager
                        .get_executor(task_with_workflow.workflow_id)
                        .await
                    {
                        Ok(executor) => {
                            executor
                                .execute(
                                    &task_with_workflow.job_name,
                                    task_with_workflow.params.as_ref().map(|p| p.0.clone()),
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
        let dispatch_ms = dispatch_start.elapsed().as_millis();
        info!("Dispatched {} tasks in {}ms", results.len(), dispatch_ms);

        // Batch update statuses
        let update_prep_start = Instant::now();
        let updates: Vec<(Uuid, &str, Option<&str>, Option<String>)> = results
            .iter()
            .map(|(task_id, result)| {
                match result {
                    Ok(execution_name) => (*task_id, "dispatched", Some(execution_name.as_str()), None),
                    Err(e) => {
                        error!("Failed to dispatch task {}: {}", task_id, e);
                        (*task_id, "failed", None, Some(e.to_string()))
                    }
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
        info!("Batch updated {} tasks in {}ms", updates_ref.len(), db_update_ms);

        Ok(())
    }

    async fn check_running_tasks(&self) -> Result<()> {
        // OPTIMIZATION: Single JOIN query instead of N+1 queries
        let tasks = self
            .db
            .get_running_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(());
        }

        // Ensure all workflows have registered executors
        let mut workflow_ids = std::collections::HashSet::new();
        for task in &tasks {
            workflow_ids.insert(task.workflow_id);
        }

        for workflow_id in workflow_ids {
            if self.executor_manager.get_executor(workflow_id).await.is_err() {
                let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = $1")
                    .bind(workflow_id)
                    .fetch_one(self.db.pool())
                    .await?;
                self.executor_manager.register_workflow(&workflow).await?;
            }
        }

        // OPTIMIZATION: Process status checks concurrently with limit
        let results = stream::iter(tasks)
            .map(|task| {
                let executor_manager = &self.executor_manager;

                async move {
                    if let Some(ref execution_name) = task.execution_name {
                        let status_result = match executor_manager
                            .get_executor(task.workflow_id)
                            .await
                        {
                            Ok(executor) => executor.get_status(execution_name).await,
                            Err(e) => {
                                warn!("Failed to get executor for task {}: {}", task.task_id, e);
                                return None;
                            }
                        };

                        match status_result {
                            Ok(status_str) => {
                                let execution_status = ExecutionStatus::from_str(&status_str);
                                let new_status = execution_status.to_task_status();
                                let old_status = task.task_status();

                                // Don't update if status is unknown and task was already completed
                                if execution_status == ExecutionStatus::Unknown
                                    && matches!(old_status, TaskStatus::Success | TaskStatus::Failed) {
                                    warn!("Task {} returned unknown status but was already completed ({}), skipping update",
                                          task.task_id, old_status.as_str());
                                    return None;
                                }

                                Some((task.task_id, task.run_id, new_status, old_status))
                            }
                            Err(e) => {
                                warn!("Failed to get status for task {}: {}", task.task_id, e);
                                None
                            }
                        }
                    } else {
                        None
                    }
                }
            })
            .buffer_unordered(self.config.max_concurrent_status_checks)
            .collect::<Vec<_>>()
            .await;

        // Batch update changed statuses
        let status_changes: Vec<_> = results
            .into_iter()
            .flatten()
            .filter(|(_, _, new_status, old_status)| new_status != old_status)
            .collect();

        if !status_changes.is_empty() {
            let updates: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = status_changes
                .iter()
                .map(|(task_id, _, new_status, _)| (*task_id, new_status.as_str(), None, None))
                .collect();

            self.db.batch_update_task_status(&updates).await?;

            for (task_id, run_id, new_status, _) in status_changes {
                info!("Task {} updated to status: {}", task_id, new_status.as_str());

                // Check run completion for finished tasks
                if matches!(new_status, TaskStatus::Success | TaskStatus::Failed) {
                    self.check_run_completion(run_id).await?;
                }
            }
        }

        Ok(())
    }

    async fn check_run_completion(&self, run_id: Uuid) -> Result<()> {
        let tasks = self.db.list_tasks(run_id).await?;

        let all_complete = tasks
            .iter()
            .all(|t| matches!(t.status(), TaskStatus::Success | TaskStatus::Failed));

        if all_complete {
            let any_failed = tasks.iter().any(|t| t.status() == TaskStatus::Failed);
            let new_status = if any_failed {
                TaskStatus::Failed
            } else {
                TaskStatus::Success
            };

            self.db
                .update_run_status(run_id, new_status.as_str(), None)
                .await?;
            info!(
                "Run {} completed with status: {}",
                run_id,
                new_status.as_str()
            );
        }

        Ok(())
    }
}
