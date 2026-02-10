use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use tracing::info;

use super::Execute;
use ork_core::config::OrchestratorConfig;
use ork_core::database::Database;
use ork_core::scheduler::Scheduler;
use ork_executors::ExecutorManager;

#[derive(Args)]
pub struct Run {
    /// Optional workflow YAML file path to execute locally
    #[arg(value_name = "WORKFLOW_FILE")]
    pub file: Option<String>,

    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Maximum time to wait for completion in seconds (only used with WORKFLOW_FILE)
    #[arg(short, long, default_value = "300")]
    pub timeout: u64,
}

impl Run {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        if let Some(file) = self.file {
            return Execute {
                file,
                config: self.config,
                timeout: self.timeout,
            }
            .execute(db)
            .await;
        }

        info!("Starting orchestrator...");
        let executor_manager = Arc::new(ExecutorManager::new());
        let scheduler = if let Some(config_path) = self.config {
            let config_content = std::fs::read_to_string(&config_path)?;
            let orchestrator_config: OrchestratorConfig = serde_yaml::from_str(&config_content)?;
            Scheduler::new_with_config(db.clone(), executor_manager, orchestrator_config)
        } else {
            Scheduler::new(db.clone(), executor_manager)
        };
        scheduler.run().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;

    #[tokio::test]
    async fn test_run_command_errors_on_missing_config_file() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let result = Run {
            file: None,
            config: Some("/tmp/does-not-exist-config.yaml".to_string()),
            timeout: 300,
        }
        .execute(db)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_command_errors_on_invalid_yaml_config() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let path = std::env::temp_dir().join(format!("ork-run-invalid-{}.yaml", Uuid::new_v4()));
        std::fs::write(&path, "this: [is: not-valid-yaml").expect("write invalid yaml");

        let result = Run {
            file: None,
            config: Some(path.to_string_lossy().to_string()),
            timeout: 300,
        }
        .execute(db)
        .await;

        let _ = std::fs::remove_file(path);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_command_uses_default_config_branch() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let handle = tokio::spawn(async move {
            Run {
                file: None,
                config: None,
                timeout: 300,
            }
            .execute(db)
            .await
        });
        sleep(Duration::from_millis(60)).await;
        handle.abort();
        let join = handle.await;
        assert!(join.is_err(), "aborted scheduler loop should cancel task");
    }

    #[tokio::test]
    async fn test_run_command_uses_config_file_branch() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let path = std::env::temp_dir().join(format!("ork-run-valid-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 10
max_concurrent_dispatches: 2
max_concurrent_status_checks: 2
db_pool_size: 2
enable_triggerer: false
"#,
        )
        .expect("write valid yaml");

        let cfg_path = path.to_string_lossy().to_string();
        let handle = tokio::spawn(async move {
            Run {
                file: None,
                config: Some(cfg_path),
                timeout: 300,
            }
            .execute(db)
            .await
        });
        sleep(Duration::from_millis(60)).await;
        handle.abort();
        let join = handle.await;
        let _ = std::fs::remove_file(path);
        assert!(join.is_err(), "aborted scheduler loop should cancel task");
    }

    #[tokio::test]
    async fn test_run_command_with_workflow_file_uses_execute_path() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let temp_dir = std::env::temp_dir().join(format!("ork-run-workflow-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let workflow_path = temp_dir.join("workflow.yaml");

        std::fs::write(
            &workflow_path,
            r#"
name: wf-run-command-success
tasks:
  task_a:
    executor: process
    command: "printf 'ORK_OUTPUT:{\"ok\":true}\n'"
"#,
        )
        .expect("write workflow");

        let result = Run {
            file: Some(workflow_path.to_string_lossy().to_string()),
            config: None,
            timeout: 10,
        }
        .execute(db)
        .await;

        let _ = std::fs::remove_dir_all(temp_dir);
        assert!(result.is_ok());
    }
}
