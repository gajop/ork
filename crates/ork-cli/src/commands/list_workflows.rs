use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct ListWorkflows;

impl ListWorkflows {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let workflows = db.list_workflows().await?;

        if workflows.is_empty() {
            println!("No workflows found");
        } else {
            println!("Workflows:");
            println!("{:<36} {:<30} {:<30}", "ID", "Name", "Cloud Run Job");
            println!("{}", "-".repeat(96));

            for wf in workflows {
                println!("{:<36} {:<30} {:<30}", wf.id, wf.name, wf.job_name);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::database::WorkflowRepository;
    use ork_state::SqliteDatabase;

    #[tokio::test]
    async fn test_list_workflows_empty_and_non_empty() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        ListWorkflows
            .execute(db.clone())
            .await
            .expect("listing empty workflows should succeed");

        db.create_workflow("wf", None, "job", "local", "local", "process", None, None)
            .await
            .expect("create workflow");

        ListWorkflows
            .execute(db)
            .await
            .expect("listing workflows should succeed");
    }
}
