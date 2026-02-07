use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct Trigger {
    /// Workflow name to trigger
    pub workflow_name: String,
}

impl Trigger {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let workflow = db.get_workflow(&self.workflow_name).await?;
        let run = db.create_run(workflow.id, "manual").await?;

        println!("✓ Triggered workflow: {}", self.workflow_name);
        println!("  Run ID: {}", run.id);
        println!("  Status: {}", run.status_str());
        println!("\nUse 'status {}' to check progress", run.id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;

    #[tokio::test]
    async fn test_trigger_command_creates_run() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        let workflow = db
            .create_workflow(
                "wf-trigger",
                None,
                "job",
                "local",
                "local",
                "process",
                None,
                None,
            )
            .await
            .expect("create workflow");

        Trigger {
            workflow_name: "wf-trigger".to_string(),
        }
        .execute(db.clone())
        .await
        .expect("trigger command should succeed");

        let runs = db.list_runs(Some(workflow.id)).await.expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].triggered_by, "manual");
    }
}
