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
