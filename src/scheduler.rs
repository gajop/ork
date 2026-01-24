use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::cloud_run::CloudRunClient;
use crate::config::OrchestratorConfig;
use crate::db::Database;
use crate::models::{ExecutorType, Workflow};
use crate::process_executor::ProcessExecutor;

pub struct Scheduler {
    db: Arc<Database>,
    config: OrchestratorConfig,
}

impl Scheduler {
    pub fn new(db: Arc<Database>) -> Self {
        Self::new_with_config(db, OrchestratorConfig::default())
    }

    pub fn new_with_config(db: Arc<Database>, config: OrchestratorConfig) -> Self {
        Self { db, config }
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
            if let Err(e) = self.process_pending_runs().await {
                error!("Error processing pending runs: {}", e);
            }

            if let Err(e) = self.process_pending_tasks().await {
                error!("Error processing pending tasks: {}", e);
            }

            if let Err(e) = self.check_running_tasks().await {
                error!("Error checking running tasks: {}", e);
            }

            sleep(Duration::from_secs(self.config.poll_interval_secs)).await;
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
        // OPTIMIZATION: Single JOIN query instead of N+1 queries
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(());
        }

        info!("Processing {} pending tasks", tasks.len());

        // OPTIMIZATION: Process tasks concurrently with limit to avoid memory explosion
        let results = stream::iter(tasks)
            .map(|task_with_workflow| async move {
                let executor_type = ExecutorType::from_str(&task_with_workflow.executor_type)
                    .unwrap_or(ExecutorType::CloudRun);

                let execution_result = match executor_type {
                    ExecutorType::CloudRun => {
                        let client = CloudRunClient::new(
                            task_with_workflow.cloud_run_project.clone(),
                            task_with_workflow.cloud_run_region.clone(),
                        );
                        client
                            .execute_job(
                                &task_with_workflow.cloud_run_job_name,
                                task_with_workflow.params.as_ref().map(|p| p.0.clone()),
                            )
                            .await
                    }
                    ExecutorType::Process => {
                        let executor = ProcessExecutor::new(Some("test-scripts".to_string()));
                        executor
                            .execute_process(
                                &task_with_workflow.cloud_run_job_name,
                                task_with_workflow.params.as_ref().map(|p| p.0.clone()),
                            )
                            .await
                    }
                };

                (task_with_workflow.task_id, execution_result)
            })
            .buffer_unordered(self.config.max_concurrent_dispatches)
            .collect::<Vec<_>>()
            .await;

        // Update statuses
        for (task_id, result) in results {
            match result {
                Ok(execution_name) => {
                    self.db
                        .update_task_status(task_id, "dispatched", Some(&execution_name), None)
                        .await?;
                }
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task_id, e);
                    self.db
                        .update_task_status(task_id, "failed", None, Some(&e.to_string()))
                        .await?;
                }
            }
        }

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

        // OPTIMIZATION: Process status checks concurrently with limit
        let results = stream::iter(tasks)
            .map(|task| async move {
                if let Some(ref execution_name) = task.cloud_run_execution_name {
                    let executor_type = ExecutorType::from_str(&task.executor_type)
                        .unwrap_or(ExecutorType::CloudRun);

                    let status_result = match executor_type {
                        ExecutorType::CloudRun => {
                            let client = CloudRunClient::new(
                                task.cloud_run_project.clone(),
                                task.cloud_run_region.clone(),
                            );
                            client.get_execution_status(execution_name).await
                        }
                        ExecutorType::Process => {
                            let executor = ProcessExecutor::new(Some("test-scripts".to_string()));
                            executor.get_process_status(execution_name).await
                        }
                    };

                    match status_result {
                        Ok(status) => {
                            let new_status = match status.as_str() {
                                "ready" | "completed" | "success" => "success",
                                "failed" | "error" => "failed",
                                _ => "running",
                            };
                            Some((task.task_id, task.run_id, new_status, task.task_status))
                        }
                        Err(e) => {
                            warn!("Failed to get status for task {}: {}", task.task_id, e);
                            None
                        }
                    }
                } else {
                    None
                }
            })
            .buffer_unordered(self.config.max_concurrent_status_checks)
            .collect::<Vec<_>>()
            .await;

        // Update changed statuses
        for result in results.into_iter().flatten() {
            let (task_id, run_id, new_status, old_status) = result;

            if new_status != old_status {
                self.db
                    .update_task_status(task_id, new_status, None, None)
                    .await?;
                info!("Task {} updated to status: {}", task_id, new_status);

                // Check run completion for finished tasks
                if new_status == "success" || new_status == "failed" {
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
            .all(|t| t.status == "success" || t.status == "failed");

        if all_complete {
            let any_failed = tasks.iter().any(|t| t.status == "failed");
            let new_status = if any_failed { "failed" } else { "success" };

            self.db.update_run_status(run_id, new_status, None).await?;
            info!("Run {} completed with status: {}", run_id, new_status);
        }

        Ok(())
    }
}
