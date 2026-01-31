use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct DeleteWorkflow {
    /// Workflow name to delete
    pub workflow_name: String,
}

impl DeleteWorkflow {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        db.delete_workflow(&self.workflow_name).await?;
        println!("✓ Deleted workflow: {}", self.workflow_name);
        Ok(())
    }
}
