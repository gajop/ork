use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct Status {
    /// Specific run ID to show (optional)
    pub run_id: Option<String>,

    /// Workflow name to filter by (optional)
    #[arg(short, long)]
    pub workflow: Option<String>,
}

impl Status {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let runs = if let Some(rid) = self.run_id {
            let run_uuid = rid.parse()?;
            vec![db.get_run(run_uuid).await?]
        } else if let Some(wf_name) = self.workflow {
            let workflow = db.get_workflow(&wf_name).await?;
            db.list_runs(Some(workflow.id)).await?
        } else {
            db.list_runs(None).await?
        };

        if runs.is_empty() {
            println!("No runs found");
        } else {
            println!("Runs:");
            println!(
                "{:<36} {:<36} {:<12} {:<20} {:<20}",
                "Run ID", "Workflow ID", "Status", "Started", "Finished"
            );
            println!("{}", "-".repeat(124));

            for run in runs {
                let started = run
                    .started_at
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string());

                let finished = run
                    .finished_at
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<36} {:<36} {:<12} {:<20} {:<20}",
                    run.id,
                    run.workflow_id,
                    run.status_str(),
                    started,
                    finished
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::database::{RunRepository, WorkflowRepository};
    use ork_state::SqliteDatabase;

    async fn setup_db() -> (Arc<SqliteDatabase>, uuid::Uuid) {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        let workflow = db
            .create_workflow(
                "wf-status",
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
        let run = db
            .create_run(workflow.id, "test")
            .await
            .expect("create run");
        (db, run.id)
    }

    #[tokio::test]
    async fn test_status_command_branches() {
        let (db, run_id) = setup_db().await;

        Status {
            run_id: Some(run_id.to_string()),
            workflow: None,
        }
        .execute(db.clone())
        .await
        .expect("status by run id should succeed");

        Status {
            run_id: None,
            workflow: Some("wf-status".to_string()),
        }
        .execute(db.clone())
        .await
        .expect("status by workflow should succeed");

        Status {
            run_id: None,
            workflow: None,
        }
        .execute(db)
        .await
        .expect("status all should succeed");
    }

    #[tokio::test]
    async fn test_status_command_prints_no_runs_for_empty_db() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        Status {
            run_id: None,
            workflow: None,
        }
        .execute(db)
        .await
        .expect("status with no runs should succeed");
    }
}
