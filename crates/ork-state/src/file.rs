use async_trait::async_trait;
use chrono::Utc;
use fs2::FileExt;
use std::path::{Path, PathBuf};
use tokio::fs;

use ork_core::error::OrkResult;
use ork_core::types::{Run, RunId, RunStatus, TaskRun};
use ork_core::workflow::Workflow;

use crate::StateStore;

#[derive(Clone)]
pub struct FileStateStore {
    base: PathBuf,
    lock_path: PathBuf,
}

impl FileStateStore {
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref().to_path_buf();
        Self {
            lock_path: base.join("state.lock"),
            base,
        }
    }

    fn run_path(&self, run_id: &str) -> PathBuf {
        self.base.join("runs").join(format!("{run_id}.json"))
    }

    fn tasks_dir(&self, run_id: &str) -> PathBuf {
        self.base.join("runs").join(run_id).join("tasks")
    }

    fn task_path(&self, run_id: &str, task: &str) -> PathBuf {
        self.tasks_dir(run_id).join(format!("{task}.json"))
    }

    fn lock(&self) -> OrkResult<std::fs::File> {
        std::fs::create_dir_all(&self.base).ok();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.lock_path)
            .map_err(|source| ork_core::error::OrkError::WriteFile {
                path: self.lock_path.clone(),
                source,
            })?;
        file.lock_exclusive()
            .map_err(|source| ork_core::error::OrkError::WriteFile {
                path: self.lock_path.clone(),
                source,
            })?;
        Ok(file)
    }
}

#[async_trait]
impl StateStore for FileStateStore {
    async fn create_run(&self, workflow: &Workflow) -> OrkResult<Run> {
        let _guard = self.lock()?;
        fs::create_dir_all(self.base.join("runs")).await.ok();
        let run = Run::new(workflow);
        let path = self.run_path(&run.id);
        let bytes = serde_json::to_vec_pretty(&run).unwrap_or_default();
        fs::write(path, bytes).await.ok();
        Ok(run)
    }

    async fn get_run(&self, run_id: &RunId) -> OrkResult<Option<Run>> {
        let _guard = self.lock()?;
        let path = self.run_path(run_id);
        match fs::read(&path).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Ok(None),
        }
    }

    async fn list_runs(&self) -> OrkResult<Vec<Run>> {
        let _guard = self.lock()?;
        let mut runs = Vec::new();
        let dir = self.base.join("runs");
        if let Ok(mut entries) = fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry
                    .file_type()
                    .await
                    .map(|ft| ft.is_file())
                    .unwrap_or(false)
                    && let Ok(bytes) = fs::read(entry.path()).await
                    && let Ok(run) = serde_json::from_slice::<Run>(&bytes)
                {
                    runs.push(run);
                }
            }
        }
        runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(runs)
    }

    async fn list_task_runs(&self, run_id: &RunId) -> OrkResult<Vec<TaskRun>> {
        let _guard = self.lock()?;
        let mut tasks = Vec::new();
        let dir = self.tasks_dir(run_id);
        if let Ok(mut entries) = fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry
                    .file_type()
                    .await
                    .map(|ft| ft.is_file())
                    .unwrap_or(false)
                    && let Ok(bytes) = fs::read(entry.path()).await
                    && let Ok(task) = serde_json::from_slice::<TaskRun>(&bytes)
                {
                    tasks.push(task);
                }
            }
        }
        Ok(tasks)
    }

    async fn upsert_task_run(&self, task_run: TaskRun) -> OrkResult<()> {
        let _guard = self.lock()?;
        let path = self.task_path(&task_run.run_id, &task_run.task);
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir).await;
        }
        let bytes = serde_json::to_vec_pretty(&task_run).unwrap_or_default();
        let _ = fs::write(path, bytes).await;
        Ok(())
    }

    async fn update_run_status(&self, run_id: &RunId, status: RunStatus) -> OrkResult<()> {
        let _guard = self.lock()?;
        let path = self.run_path(run_id);
        if let Ok(bytes) = fs::read(&path).await
            && let Ok(mut run) = serde_json::from_slice::<Run>(&bytes)
        {
            if run.started_at.is_none() && status == RunStatus::Running {
                run.started_at = Some(Utc::now());
            }
            if matches!(
                status,
                RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled
            ) {
                run.finished_at = Some(Utc::now());
            }
            run.status = status;
            let bytes = serde_json::to_vec_pretty(&run).unwrap_or_default();
            let _ = fs::write(path, bytes).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use ork_core::types::{TaskRun, TaskStatus};
    use ork_core::workflow::{ExecutorKind, TaskDefinition, Workflow};
    use uuid::Uuid;

    fn sample_workflow() -> Workflow {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "task1".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo hi".to_string()),
                job: None,
                module: None,
                function: None,
                inputs: serde_json::Value::Null,
                depends_on: vec![],
                timeout: 60,
                retries: 0,
                input_type: None,
                output_type: None,
            },
        );
        Workflow {
            name: "wf".to_string(),
            description: None,
            schedule: None,
            types: std::collections::BTreeMap::new(),
            tasks,
        }
    }

    fn temp_store() -> (FileStateStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ork-file-store-{}", Uuid::new_v4()));
        (FileStateStore::new(&dir), dir)
    }

    #[tokio::test]
    async fn test_create_get_and_list_runs() {
        let (store, dir) = temp_store();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");

        let fetched = store.get_run(&run.id).await.expect("get run");
        assert!(fetched.is_some());

        let runs = store.list_runs().await.expect("list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run.id);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_upsert_and_list_task_runs() {
        let (store, dir) = temp_store();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");
        let task = TaskRun {
            run_id: run.id.clone(),
            task: "task1".to_string(),
            status: TaskStatus::Running,
            attempt: 1,
            max_retries: 2,
            created_at: chrono::Utc::now(),
            dispatched_at: None,
            started_at: Some(chrono::Utc::now()),
            finished_at: None,
            error: None,
            output: None,
        };
        store
            .upsert_task_run(task)
            .await
            .expect("upsert task should succeed");

        let listed = store.list_task_runs(&run.id).await.expect("list task runs");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].task, "task1");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_update_run_status_updates_fields() {
        let (store, dir) = temp_store();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");

        store
            .update_run_status(&run.id, RunStatus::Running)
            .await
            .expect("set running");
        store
            .update_run_status(&run.id, RunStatus::Failed)
            .await
            .expect("set failed");

        let fetched = store
            .get_run(&run.id)
            .await
            .expect("get run")
            .expect("run exists");
        assert_eq!(fetched.status, RunStatus::Failed);
        assert!(fetched.started_at.is_some());
        assert!(fetched.finished_at.is_some());

        let _ = std::fs::remove_dir_all(dir);
    }
}
