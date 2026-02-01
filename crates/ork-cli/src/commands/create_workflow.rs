use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct CreateWorkflow {
    /// Workflow name
    #[arg(short, long)]
    pub name: String,

    /// Description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Cloud Run job name (or script path for process executor)
    #[arg(short, long)]
    pub job_name: String,

    /// Cloud Run project ID (or 'local' for process executor)
    #[arg(short, long)]
    pub project: String,

    /// Cloud Run region (or 'local' for process executor)
    #[arg(short, long, default_value = "us-central1")]
    pub region: String,

    /// Number of tasks to generate per run
    #[arg(short, long, default_value = "3")]
    pub task_count: i32,

    /// Executor type: cloudrun or process
    #[arg(short, long, default_value = "cloudrun")]
    pub executor: String,
}

impl CreateWorkflow {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let task_params = serde_json::json!({
            "task_count": self.task_count,
        });

        let workflow = db
            .create_workflow(
                &self.name,
                self.description.as_deref(),
                &self.job_name,
                &self.region,
                &self.project,
                &self.executor,
                Some(task_params),
                None,
            )
            .await?;

        println!("✓ Created workflow: {}", workflow.name);
        println!("  ID: {}", workflow.id);
        println!("  Job/Script: {}", workflow.job_name);
        println!("  Executor: {}", workflow.executor_type);
        println!("  Project: {}", workflow.project);
        println!("  Region: {}", workflow.region);
        Ok(())
    }
}
