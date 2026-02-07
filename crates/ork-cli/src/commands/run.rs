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
}
