// FileDatabase - File-based implementation of the Database trait
//
// This stores workflows, runs, and tasks as JSON files in a directory structure:
// - workflows/{workflow_id}.json
// - runs/{run_id}.json
// - tasks/{task_id}.json

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

use ork_core::database::Database;
use ork_core::models::{Run, Task, TaskWithWorkflow, Workflow};

#[derive(Clone)]
pub struct FileDatabase {
    base: PathBuf,
}

impl FileDatabase {
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self { base }
    }

    fn workflows_dir(&self) -> PathBuf {
        self.base.join("workflows")
    }

    fn workflow_path(&self, id: Uuid) -> PathBuf {
        self.workflows_dir().join(format!("{}.json", id))
    }

    fn runs_dir(&self) -> PathBuf {
        self.base.join("runs")
    }

    fn run_path(&self, id: Uuid) -> PathBuf {
        self.runs_dir().join(format!("{}.json", id))
    }

    fn tasks_dir(&self) -> PathBuf {
        self.base.join("tasks")
    }

    fn task_path(&self, id: Uuid) -> PathBuf {
        self.tasks_dir().join(format!("{}.json", id))
    }

    async fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.workflows_dir()).await?;
        fs::create_dir_all(self.runs_dir()).await?;
        fs::create_dir_all(self.tasks_dir()).await?;
        Ok(())
    }

    async fn write_json<T: Serialize>(&self, path: &Path, data: &T) -> Result<()> {
        let json = serde_json::to_string_pretty(data)?;
        fs::write(path, json).await?;
        Ok(())
    }

    async fn read_json<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Result<T> {
        let contents = fs::read_to_string(path).await?;
        Ok(serde_json::from_str(&contents)?)
    }
}

#[async_trait]
impl Database for FileDatabase {
    async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
    ) -> Result<Workflow> {
        self.ensure_dirs().await?;

        let workflow = Workflow {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            job_name: job_name.to_string(),
            region: region.to_string(),
            project: project.to_string(),
            executor_type: executor_type.to_string(),
            task_params: task_params,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.write_json(&self.workflow_path(workflow.id), &workflow).await?;
        Ok(workflow)
    }

    async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        // Need to scan all workflows to find by name
        let mut entries = fs::read_dir(self.workflows_dir()).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(workflow) = self.read_json::<Workflow>(&entry.path()).await {
                if workflow.name == name {
                    return Ok(workflow);
                }
            }
        }

        anyhow::bail!("Workflow not found: {}", name)
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        self.ensure_dirs().await?;

        let mut workflows = Vec::new();
        let mut entries = fs::read_dir(self.workflows_dir()).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(workflow) = self.read_json::<Workflow>(&entry.path()).await {
                workflows.push(workflow);
            }
        }

        workflows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(workflows)
    }

    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        self.ensure_dirs().await?;

        let id = Uuid::new_v4();
        let now = Utc::now();

        // Use serde_json to construct Run with private fields
        let run: Run = serde_json::from_value(serde_json::json!({
            "id": id,
            "workflow_id": workflow_id,
            "status": "pending",
            "triggered_by": triggered_by,
            "started_at": null,
            "finished_at": null,
            "error": null,
            "created_at": now,
        }))?;

        self.write_json(&self.run_path(run.id), &run).await?;
        Ok(run)
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.run_path(run_id);
        let run: Run = self.read_json(&path).await?;

        let now = Utc::now();
        let started_at = if status == "running" && run.started_at.is_none() {
            Some(now)
        } else {
            run.started_at
        };

        let finished_at = if matches!(status, "success" | "failed") && run.finished_at.is_none() {
            Some(now)
        } else {
            run.finished_at
        };

        // Reconstruct with updated fields using serde
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

    async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        self.read_json(&self.run_path(run_id)).await
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
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

    async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        let all_runs = self.list_runs(None).await?;
        Ok(all_runs.into_iter().filter(|r| r.status_str() == "pending").collect())
    }

    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        _workflow_name: &str,
    ) -> Result<()> {
        self.ensure_dirs().await?;

        for i in 0..task_count {
            let id = Uuid::new_v4();
            let now = Utc::now();

            // Use serde_json to construct Task with private fields
            let task: Task = serde_json::from_value(serde_json::json!({
                "id": id,
                "run_id": run_id,
                "task_index": i,
                "status": "pending",
                "execution_name": null,
                "params": null,
                "output": null,
                "error": null,
                "dispatched_at": null,
                "started_at": null,
                "finished_at": null,
                "created_at": now,
            }))?;

            self.write_json(&self.task_path(task.id), &task).await?;
        }

        Ok(())
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let path = self.task_path(task_id);
        let task: Task = self.read_json(&path).await?;

        let now = Utc::now();
        let dispatched_at = if status == "dispatched" && task.dispatched_at.is_none() {
            Some(now)
        } else {
            task.dispatched_at
        };

        let started_at = if status == "running" && task.started_at.is_none() {
            Some(now)
        } else {
            task.started_at
        };

        let finished_at = if matches!(status, "success" | "failed") && task.finished_at.is_none() {
            Some(now)
        } else {
            task.finished_at
        };

        // Reconstruct with updated fields using serde
        let updated_task: Task = serde_json::from_value(serde_json::json!({
            "id": task.id,
            "run_id": task.run_id,
            "task_index": task.task_index,
            "status": status,
            "execution_name": execution_name.or(task.execution_name.as_deref()),
            "params": task.params,
            "output": task.output,
            "error": error.or(task.error.as_deref()),
            "dispatched_at": dispatched_at,
            "started_at": started_at,
            "finished_at": finished_at,
            "created_at": task.created_at,
        }))?;

        self.write_json(&path, &updated_task).await?;
        Ok(())
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        for (task_id, status, execution_name, error) in updates {
            self.update_task_status(*task_id, status, *execution_name, *error).await?;
        }
        Ok(())
    }

    async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;

        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await {
                if task.run_id == run_id {
                    tasks.push(task);
                }
            }
        }

        tasks.sort_by_key(|t| t.task_index);
        Ok(tasks)
    }

    async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;

        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await {
                if task.status_str() == "pending" {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }

    async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        self.ensure_dirs().await?;

        let mut tasks = Vec::new();
        let mut entries = fs::read_dir(self.tasks_dir()).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Ok(task) = self.read_json::<Task>(&entry.path()).await {
                if matches!(task.status_str(), "running" | "dispatched") {
                    tasks.push(task);
                }
            }
        }

        Ok(tasks)
    }

    async fn get_pending_tasks_with_workflow(&self, _limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        // TaskWithWorkflow has private fields and no public constructor
        // This method is only used by the scheduler for optimization
        // FileDatabase doesn't need this optimization since it's file-based
        anyhow::bail!("FileDatabase::get_pending_tasks_with_workflow not yet implemented - use get_pending_tasks instead");
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        let mut workflows = Vec::new();

        for id in workflow_ids {
            if let Ok(workflow) = self.read_json::<Workflow>(&self.workflow_path(*id)).await {
                workflows.push(workflow);
            }
        }

        Ok(workflows)
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        self.read_json(&self.workflow_path(workflow_id)).await
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        let task: Task = self.read_json(&self.task_path(task_id)).await?;
        Ok(task.run_id)
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let tasks = self.list_tasks(run_id).await?;

        let total = tasks.len() as i64;
        let completed = tasks.iter().filter(|t| matches!(t.status_str(), "success" | "failed")).count() as i64;
        let failed = tasks.iter().filter(|t| t.status_str() == "failed").count() as i64;

        Ok((total, completed, failed))
    }
}
