use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{Duration, timeout};
use tracing::info;

use ork_core::config::OrchestratorConfig;
use ork_core::database::Database;
use ork_core::models::{RunStatus, TaskStatus};
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

        let yaml: serde_yaml::Value =
            serde_yaml::from_str(&yaml_content).with_context(|| "Failed to parse YAML")?;

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
                enable_triggerer: true,
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
                let tasks = db.list_tasks(run.id).await?;

                let success_count = tasks
                    .iter()
                    .filter(|t| matches!(t.status(), TaskStatus::Success))
                    .count();
                let failed_count = tasks
                    .iter()
                    .filter(|t| matches!(t.status(), TaskStatus::Failed))
                    .count();
                let run_status = if failed_count > 0 {
                    RunStatus::Failed
                } else {
                    RunStatus::Success
                };

                let run_info = db.get_run(run.id).await?;
                if !matches!(run_info.status(), RunStatus::Success | RunStatus::Failed) {
                    let _ = db.update_run_status(run.id, run_status, None).await;
                }

                if matches!(run_status, RunStatus::Success) {
                    println!(
                        "✓ Run completed successfully in {:.2}s",
                        elapsed.as_secs_f64()
                    );
                } else {
                    println!("✗ Run failed in {:.2}s", elapsed.as_secs_f64());
                }

                println!(
                    "  Tasks: {} success, {} failed, {} total",
                    success_count,
                    failed_count,
                    tasks.len()
                );
                println!();

                // Show task details
                let format_dt = |dt: chrono::DateTime<chrono::Utc>| {
                    let base = dt.format("%Y-%m-%dT%H:%M:%S").to_string();
                    let centis = dt.timestamp_subsec_millis() / 10;
                    format!("{base}.{:02}Z", centis)
                };

                println!("Tasks:");
                println!(
                    "{:<20} {:<15} {:<10} {:<8} {:<24} {:<24}",
                    "Name", "Status", "Duration", "Attempts", "Started", "Finished"
                );
                println!("{}", "-".repeat(105));

                for task in &tasks {
                    let duration = if let (Some(started), Some(finished)) =
                        (task.started_at, task.finished_at)
                    {
                        let duration = finished.signed_duration_since(started);
                        format!("{:.2}s", duration.num_milliseconds() as f64 / 1000.0)
                    } else {
                        "-".to_string()
                    };
                    let status = task.status();
                    let attempt_count = match status {
                        TaskStatus::Success | TaskStatus::Running | TaskStatus::Dispatched => {
                            task.attempts + 1
                        }
                        TaskStatus::Failed => task.attempts,
                        _ => task.attempts,
                    };
                    let attempts_display = if attempt_count > 0 {
                        attempt_count.to_string()
                    } else {
                        "-".to_string()
                    };
                    let started = task
                        .started_at
                        .map(format_dt)
                        .unwrap_or_else(|| "-".to_string());
                    let finished = task
                        .finished_at
                        .map(format_dt)
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<20} {:<15} {:<10} {:<8} {:<24} {:<24}",
                        task.task_name,
                        status.as_str(),
                        duration,
                        attempts_display,
                        started,
                        finished
                    );
                }

                if !matches!(run_status, RunStatus::Success) {
                    return Err(anyhow!("Run completed with failed status"));
                }

                Ok(())
            }
            Ok(Err(e)) => {
                println!("✗ Error during execution: {}", e);
                Err(e)
            }
            Err(_) => {
                let msg = format!(
                    "Timeout waiting for workflow to complete after {}s",
                    self.timeout
                );
                println!("✗ {msg}");
                Err(anyhow!(msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::database::{RunRepository, WorkflowRepository};
    use ork_state::SqliteDatabase;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_execute_command_errors_on_missing_workflow_file() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let result = Execute {
            file: "/tmp/does-not-exist-workflow.yaml".to_string(),
            config: None,
            timeout: 1,
        }
        .execute(db)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_command_success_path() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let temp_dir = std::env::temp_dir().join(format!("ork-execute-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let workflow_path = temp_dir.join("workflow.yaml");
        let config_path = temp_dir.join("orchestrator.yaml");

        std::fs::write(
            &workflow_path,
            r#"
name: wf-execute-success
tasks:
  task_a:
    executor: process
    command: "printf 'ORK_OUTPUT:{\"ok\":true}\n'"
"#,
        )
        .expect("write workflow");
        std::fs::write(
            &config_path,
            r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
        )
        .expect("write config");

        let result = Execute {
            file: workflow_path.to_string_lossy().to_string(),
            config: Some(config_path.to_string_lossy().to_string()),
            timeout: 10,
        }
        .execute(db.clone())
        .await;

        assert!(result.is_ok());
        let workflow = db
            .get_workflow("wf-execute-success")
            .await
            .expect("workflow should exist");
        let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status_str(), "success");

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_execute_command_replaces_existing_workflow_name() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        db.create_workflow(
            "wf-execute-replace",
            Some("old"),
            "job",
            "local",
            "local",
            "process",
            None,
            None,
        )
        .await
        .expect("pre-create workflow");

        let temp_dir = std::env::temp_dir().join(format!("ork-execute-replace-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let workflow_path = temp_dir.join("workflow.yaml");
        let config_path = temp_dir.join("orchestrator.yaml");

        std::fs::write(
            &workflow_path,
            r#"
name: wf-execute-replace
tasks:
  step:
    executor: process
    command: "printf 'ORK_OUTPUT:{\"ok\":true}\n'"
"#,
        )
        .expect("write workflow");
        std::fs::write(
            &config_path,
            r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
        )
        .expect("write config");

        let result = Execute {
            file: workflow_path.to_string_lossy().to_string(),
            config: Some(config_path.to_string_lossy().to_string()),
            timeout: 10,
        }
        .execute(db.clone())
        .await;
        assert!(result.is_ok());

        let workflows = db.list_workflows().await.expect("list workflows");
        let matching: Vec<_> = workflows
            .into_iter()
            .filter(|w| w.name == "wf-execute-replace")
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].executor_type, "dag");

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_execute_command_returns_error_on_failed_run() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let temp_dir = std::env::temp_dir().join(format!("ork-execute-failed-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let workflow_path = temp_dir.join("workflow.yaml");
        let config_path = temp_dir.join("orchestrator.yaml");

        std::fs::write(
            &workflow_path,
            r#"
name: wf-execute-failed
tasks:
  step:
    executor: process
    command: "exit 1"
"#,
        )
        .expect("write workflow");
        std::fs::write(
            &config_path,
            r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 100
max_concurrent_dispatches: 10
max_concurrent_status_checks: 50
db_pool_size: 5
enable_triggerer: false
"#,
        )
        .expect("write config");

        let err = Execute {
            file: workflow_path.to_string_lossy().to_string(),
            config: Some(config_path.to_string_lossy().to_string()),
            timeout: 10,
        }
        .execute(db)
        .await
        .expect_err("failed run should return error");
        assert!(err.to_string().contains("failed status"));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_execute_command_without_config_can_timeout() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let temp_dir = std::env::temp_dir().join(format!("ork-execute-timeout-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let workflow_path = temp_dir.join("workflow.yaml");
        std::fs::write(
            &workflow_path,
            r#"
name: wf-execute-timeout
tasks:
  step:
    executor: process
    command: "printf 'ORK_OUTPUT:{\"ok\":true}\n'"
"#,
        )
        .expect("write workflow");

        let err = Execute {
            file: workflow_path.to_string_lossy().to_string(),
            config: None,
            timeout: 0,
        }
        .execute(db)
        .await
        .expect_err("timeout should return error");
        assert!(
            err.to_string()
                .contains("Timeout waiting for workflow to complete")
        );

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
