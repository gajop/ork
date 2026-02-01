use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{timeout, Duration};
use tracing::info;

use ork_core::config::OrchestratorConfig;
use ork_core::database::Database;
use ork_core::scheduler::Scheduler;
use ork_executors::ExecutorManager;

#[derive(Args)]
pub struct Execute {
    /// Path to workflow YAML file
    #[arg(short, long)]
    pub file: String,

    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Maximum time to wait for completion in seconds
    #[arg(short, long, default_value = "300")]
    pub timeout: u64,
}

impl Execute {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let start_time = Instant::now();

        // Read and create workflow from YAML
        info!("Creating workflow from {}", self.file);
        let yaml_content = std::fs::read_to_string(&self.file)
            .with_context(|| format!("Failed to read workflow file: {}", self.file))?;

        let yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
            .with_context(|| "Failed to parse YAML")?;

        let workflow_name = yaml
            .get("name")
            .and_then(|v| v.as_str())
            .context("Workflow YAML must have a 'name' field")?;

        // Check if workflow already exists, delete if so
        let existing_workflows = db.list_workflows().await?;
        if existing_workflows.iter().any(|w| w.name == workflow_name) {
            info!("Workflow '{}' already exists, deleting...", workflow_name);
            db.delete_workflow(workflow_name).await?;
        }

        // Create workflow
        let workflow = crate::commands::create_workflow_yaml::create_workflow_from_yaml_str(
            &*db,
            &yaml_content,
            &self.file,
            "local",
            "local",
        )
        .await
        .context("Failed to create workflow")?;

        println!("✓ Created workflow: {}", workflow.name);
        println!("  ID: {}", workflow.id);

        // Trigger workflow
        info!("Triggering workflow...");
        let run = db.create_run(workflow.id, "cli-execute").await?;
        println!("✓ Triggered run: {}", run.id);
        println!();

        // Start scheduler in background with fast config for local execution
        info!("Starting scheduler...");
        let executor_manager = Arc::new(ExecutorManager::new());
        let scheduler = if let Some(config_path) = self.config {
            let config_content = std::fs::read_to_string(&config_path)?;
            let orchestrator_config: OrchestratorConfig = serde_yaml::from_str(&config_content)?;
            Scheduler::new_with_config(db.clone(), executor_manager, orchestrator_config)
        } else {
            // Use fast config for local execution - 10ms poll interval
            let config = OrchestratorConfig {
                poll_interval_secs: 0.01,
                max_tasks_per_batch: 100,
                max_concurrent_dispatches: 10,
                max_concurrent_status_checks: 50,
                db_pool_size: 5,
            };
            Scheduler::new_with_config(db.clone(), executor_manager, config)
        };

        let scheduler_handle = tokio::spawn(async move {
            let _ = scheduler.run().await;
        });

        // Wait for completion
        let result = timeout(Duration::from_secs(self.timeout), async {
            loop {
                let (total, completed, failed) = db.get_run_task_stats(run.id).await?;
                if completed + failed >= total && total > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok::<(), anyhow::Error>(())
        })
        .await;

        scheduler_handle.abort();

        match result {
            Ok(Ok(())) => {
                let elapsed = start_time.elapsed();
                let run_info = db.get_run(run.id).await?;
                let tasks = db.list_tasks(run.id).await?;

                let success_count = tasks.iter().filter(|t| t.status_str() == "success").count();
                let failed_count = tasks.iter().filter(|t| t.status_str() == "failed").count();

                if run_info.status_str() == "success" {
                    println!("✓ Run completed successfully in {:.2}s", elapsed.as_secs_f64());
                } else {
                    println!("✗ Run failed in {:.2}s", elapsed.as_secs_f64());
                }

                println!("  Tasks: {} success, {} failed, {} total", success_count, failed_count, tasks.len());
                println!();

                // Show task details
                println!("Tasks:");
                println!("{:<20} {:<15} {:<10}", "Name", "Status", "Duration");
                println!("{}", "-".repeat(50));

                for task in &tasks {
                    let duration = if let (Some(started), Some(finished)) = (task.started_at, task.finished_at) {
                        let duration = finished.signed_duration_since(started);
                        format!("{:.3}s", duration.num_milliseconds() as f64 / 1000.0)
                    } else {
                        "-".to_string()
                    };

                    println!("{:<20} {:<15} {:<10}", task.task_name, task.status_str(), duration);
                }

                if run_info.status_str() != "success" {
                    std::process::exit(1);
                }

                Ok(())
            }
            Ok(Err(e)) => {
                println!("✗ Error during execution: {}", e);
                Err(e)
            }
            Err(_) => {
                println!("✗ Timeout waiting for workflow to complete after {}s", self.timeout);
                std::process::exit(1);
            }
        }
    }
}
