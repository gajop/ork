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

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::database::WorkflowRepository;
    use ork_state::SqliteDatabase;

    #[tokio::test]
    async fn test_create_workflow_command() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let cmd = CreateWorkflow {
            name: "wf".to_string(),
            description: Some("desc".to_string()),
            job_name: "job".to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
            task_count: 3,
            executor: "process".to_string(),
        };
        cmd.execute(db.clone())
            .await
            .expect("create workflow command should succeed");

        let workflow = db.get_workflow("wf").await.expect("workflow should exist");
        assert_eq!(workflow.name, "wf");
        assert_eq!(workflow.executor_type, "process");
    }
}
