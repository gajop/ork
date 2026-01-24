use anyhow::Result;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::cloud_run::CloudRunClient;
use crate::db::Database;
use crate::models::Workflow;

pub struct Scheduler {
    db: Arc<Database>,
}

impl Scheduler {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting scheduler loop");

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

            sleep(Duration::from_secs(5)).await;
        }
    }

    async fn process_pending_runs(&self) -> Result<()> {
        let runs = self.db.get_pending_runs().await?;

        for run in runs {
            info!("Processing pending run: {}", run.id);

            // Get the workflow
            let workflow = sqlx::query_as::<_, Workflow>(
                "SELECT * FROM workflows WHERE id = $1",
            )
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
        let tasks = self.db.get_pending_tasks().await?;

        for task in tasks {
            info!("Dispatching task: {} (index {})", task.id, task.task_index);

            // Get run and workflow info
            let run = self.db.get_run(task.run_id).await?;
            let workflow = sqlx::query_as::<_, Workflow>(
                "SELECT * FROM workflows WHERE id = $1",
            )
            .bind(run.workflow_id)
            .fetch_one(self.db.pool())
            .await?;

            // Create Cloud Run client
            let client = CloudRunClient::new(
                workflow.cloud_run_project.clone(),
                workflow.cloud_run_region.clone(),
            );

            // Execute the job
            match client
                .execute_job(
                    &workflow.cloud_run_job_name,
                    task.params.as_ref().map(|p| p.0.clone()),
                )
                .await
            {
                Ok(execution_name) => {
                    self.db
                        .update_task_status(task.id, "dispatched", Some(&execution_name), None)
                        .await?;
                    info!("Task {} dispatched as {}", task.id, execution_name);
                }
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task.id, e);
                    self.db
                        .update_task_status(task.id, "failed", None, Some(&e.to_string()))
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn check_running_tasks(&self) -> Result<()> {
        let tasks = self.db.get_running_tasks().await?;

        for task in tasks {
            if let Some(execution_name) = &task.cloud_run_execution_name {
                // Get run and workflow info
                let run = self.db.get_run(task.run_id).await?;
                let workflow = sqlx::query_as::<_, Workflow>(
                    "SELECT * FROM workflows WHERE id = $1",
                )
                .bind(run.workflow_id)
                .fetch_one(self.db.pool())
                .await?;

                let client = CloudRunClient::new(
                    workflow.cloud_run_project.clone(),
                    workflow.cloud_run_region.clone(),
                );

                match client.get_execution_status(execution_name).await {
                    Ok(status) => {
                        let new_status = match status.as_str() {
                            "ready" | "completed" | "success" => "success",
                            "failed" | "error" => "failed",
                            _ => "running",
                        };

                        if new_status != task.status {
                            self.db
                                .update_task_status(task.id, new_status, None, None)
                                .await?;
                            info!("Task {} updated to status: {}", task.id, new_status);
                        }

                        // Check if all tasks in the run are complete
                        if new_status == "success" || new_status == "failed" {
                            self.check_run_completion(task.run_id).await?;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get status for task {}: {}", task.id, e);
                    }
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
