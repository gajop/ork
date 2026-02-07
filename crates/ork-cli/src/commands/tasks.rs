use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::Database;

#[derive(Args)]
pub struct Tasks {
    /// Run ID
    pub run_id: String,
}

impl Tasks {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let run_id = self.run_id.parse()?;
        let tasks = db.list_tasks(run_id).await?;

        if tasks.is_empty() {
            println!("No tasks found for run {}", run_id);
        } else {
            println!("Tasks for run {}:", run_id);
            println!(
                "{:<5} {:<24} {:<36} {:<12} {:<40} {:<20}",
                "Index", "Name", "Task ID", "Status", "Execution", "Finished"
            );
            println!("{}", "-".repeat(149));

            for task in tasks {
                let execution = task.execution_name.as_deref().unwrap_or("-");

                let finished = task
                    .finished_at
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<5} {:<24} {:<36} {:<12} {:<40} {:<20}",
                    task.task_index,
                    task.task_name,
                    task.id,
                    task.status_str(),
                    execution,
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
    use ork_core::database::NewTask;
    use ork_state::SqliteDatabase;

    #[tokio::test]
    async fn test_tasks_command_empty_and_non_empty() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");
        let workflow = db
            .create_workflow(
                "wf-tasks", None, "job", "local", "local", "process", None, None,
            )
            .await
            .expect("create workflow");
        let run = db
            .create_run(workflow.id, "test")
            .await
            .expect("create run");

        Tasks {
            run_id: run.id.to_string(),
        }
        .execute(db.clone())
        .await
        .expect("tasks command should handle empty task list");

        db.batch_create_dag_tasks(
            run.id,
            &[NewTask {
                task_index: 0,
                task_name: "task-a".to_string(),
                executor_type: "process".to_string(),
                depends_on: vec![],
                params: serde_json::json!({"command":"echo hi"}),
                max_retries: 0,
                timeout_seconds: Some(10),
            }],
        )
        .await
        .expect("create task");

        Tasks {
            run_id: run.id.to_string(),
        }
        .execute(db)
        .await
        .expect("tasks command should handle populated task list");
    }
}
