use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use tracing::info;

use ork_core::config::OrchestratorConfig;
use ork_core::database::Database;
use ork_core::scheduler::Scheduler;
use ork_executors::ExecutorManager;

#[derive(Args)]
pub struct Run {
    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,
}

impl Run {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
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
            config: Some("/tmp/does-not-exist-config.yaml".to_string()),
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
            config: Some(path.to_string_lossy().to_string()),
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

        let handle = tokio::spawn(async move { Run { config: None }.execute(db).await });
        sleep(Duration::from_millis(60)).await;
        handle.abort();
        let join = handle.await;
        assert!(join.is_err(), "aborted scheduler loop should cancel task");
    }
}
