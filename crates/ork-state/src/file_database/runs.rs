use anyhow::Result;
use chrono::Utc;
use ork_core::database::{RunListEntry, RunListPage, RunListQuery};
use tokio::fs;
use uuid::Uuid;

use ork_core::models::{Run, RunStatus, TaskStatus};

use super::core::FileDatabase;

impl FileDatabase {
    pub(super) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        self.ensure_dirs().await?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let run: Run = serde_json::from_value(serde_json::json!({
            "id": id,
            "workflow_id": workflow_id,
            "status": RunStatus::Pending,
            "triggered_by": triggered_by,
            "started_at": null,
            "finished_at": null,
            "error": null,
            "created_at": now,
        }))?;
        self.write_json(&self.run_path(run.id), &run).await?;
        Ok(run)
    }

    pub(super) async fn update_run_status_impl(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.run_path(run_id);
        let run: Run = self.read_json(&path).await?;
        let now = Utc::now();
        let started_at = if matches!(status, RunStatus::Running) && run.started_at.is_none() {
            Some(now)
        } else {
            run.started_at
        };
        let finished_at = if status.is_terminal() && run.finished_at.is_none() {
            Some(now)
        } else {
            run.finished_at
        };
        let updated_run: Run = serde_json::from_value(serde_json::json!({
            "id": run.id,
            "workflow_id": run.workflow_id,
            "status": status,
            "triggered_by": run.triggered_by,
            "started_at": started_at,
            "finished_at": finished_at,
            "error": error,
            "created_at": run.created_at,
        }))?;
        self.write_json(&path, &updated_run).await?;
        Ok(())
    }

    pub(super) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        self.read_json(&self.run_path(run_id)).await
    }

    pub(super) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        self.ensure_dirs().await?;
        let mut runs = Vec::new();
        let mut entries = fs::read_dir(self.runs_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(run) = self.read_json::<Run>(&entry.path()).await {
                if workflow_id.is_none() || workflow_id == Some(run.workflow_id) {
                    runs.push(run);
                }
            }
        }
        runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(runs)
    }

    pub(super) async fn list_runs_page_impl(&self, query: &RunListQuery) -> Result<RunListPage> {
        let mut runs = self.list_runs_impl(None).await?;
        let workflows = self.list_workflows_impl().await?;
        let workflow_name_by_id = workflows
            .into_iter()
            .map(|workflow| (workflow.id, workflow.name))
            .collect::<std::collections::HashMap<_, _>>();

        if let Some(status) = query.status {
            runs.retain(|run| run.status() == status);
        }
        if let Some(workflow_name) = query.workflow_name.as_deref() {
            runs.retain(|run| {
                workflow_name_by_id
                    .get(&run.workflow_id)
                    .is_some_and(|name| name == workflow_name)
            });
        }

        runs.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });

        let total = runs.len();
        let items = runs
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .map(|run| RunListEntry {
                workflow_name: workflow_name_by_id.get(&run.workflow_id).cloned(),
                run,
            })
            .collect();

        Ok(RunListPage { items, total })
    }

    pub(super) async fn get_pending_runs_impl(&self) -> Result<Vec<Run>> {
        let all_runs = self.list_runs_impl(None).await?;
        Ok(all_runs
            .into_iter()
            .filter(|r| matches!(r.status(), RunStatus::Pending))
            .collect())
    }

    pub(super) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        let run = self.get_run_impl(run_id).await?;
        if !run.status().is_terminal() {
            self.update_run_status_impl(run_id, RunStatus::Cancelled, None)
                .await?;
        }
        let tasks = self.list_tasks_impl(run_id).await?;
        for task in tasks {
            if matches!(
                task.status(),
                TaskStatus::Pending | TaskStatus::Dispatched | TaskStatus::Running
            ) {
                self.update_task_status_impl(task.id, TaskStatus::Cancelled, None, None)
                    .await?;
            }
        }
        Ok(())
    }
}
