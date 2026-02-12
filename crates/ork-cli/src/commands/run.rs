use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use super::Execute;
use ork_core::database::Database;

#[derive(Args)]
pub struct Run {
    /// Workflow YAML file path to execute locally
    #[arg(value_name = "WORKFLOW_FILE")]
    pub file: String,

    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Maximum time to wait for completion in seconds
    #[arg(short, long, default_value = "300")]
    pub timeout: u64,
}

impl Run {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        Execute {
            file: self.file,
            config: self.config,
            timeout: self.timeout,
        }
        .execute(db)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_run_command_with_workflow_file() {
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
            file: workflow_path.to_string_lossy().to_string(),
            config: None,
            timeout: 10,
        }
        .execute(db)
        .await;

        let _ = std::fs::remove_dir_all(temp_dir);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_command_errors_on_missing_workflow_file() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let result = Run {
            file: "/tmp/does-not-exist-workflow.yaml".to_string(),
            config: None,
            timeout: 300,
        }
        .execute(db)
        .await;

        assert!(result.is_err());
    }
}
