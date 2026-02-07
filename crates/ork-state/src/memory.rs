use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use chrono::Utc;
use ork_core::error::OrkResult;
use ork_core::types::{Run, RunId, RunStatus, TaskRun};
use ork_core::workflow::Workflow;

use crate::StateStore;

#[derive(Debug, Default, Clone)]
pub struct InMemoryStateStore {
    runs: Arc<RwLock<HashMap<RunId, Run>>>,
    tasks: Arc<RwLock<HashMap<(RunId, String), TaskRun>>>,
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn create_run(&self, workflow: &Workflow) -> OrkResult<Run> {
        let run = Run::new(workflow);
        self.runs.write().await.insert(run.id.clone(), run.clone());
        Ok(run)
    }

    async fn get_run(&self, run_id: &RunId) -> OrkResult<Option<Run>> {
        Ok(self.runs.read().await.get(run_id).cloned())
    }

    async fn list_runs(&self) -> OrkResult<Vec<Run>> {
        Ok(self.runs.read().await.values().cloned().collect())
    }

    async fn list_task_runs(&self, run_id: &RunId) -> OrkResult<Vec<TaskRun>> {
        let tasks = self
            .tasks
            .read()
            .await
            .iter()
            .filter_map(|((r, _), t)| if r == run_id { Some(t.clone()) } else { None })
            .collect();
        Ok(tasks)
    }

    async fn upsert_task_run(&self, task_run: TaskRun) -> OrkResult<()> {
        let key = (task_run.run_id.clone(), task_run.task.clone());
        self.tasks.write().await.insert(key, task_run);
        Ok(())
    }

    async fn update_run_status(&self, run_id: &RunId, status: RunStatus) -> OrkResult<()> {
        if let Some(run) = self.runs.write().await.get_mut(run_id) {
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
                input: serde_json::Value::Null,
                depends_on: vec![],
                timeout: 60,
                retries: 0,
            },
        );
        Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_run() {
        let store = InMemoryStateStore::default();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");

        let fetched = store.get_run(&run.id).await.expect("get run");
        assert!(fetched.is_some());
        assert_eq!(fetched.expect("run exists").workflow, workflow.name);
    }

    #[tokio::test]
    async fn test_upsert_and_list_task_runs() {
        let store = InMemoryStateStore::default();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");
        let task_run = TaskRun {
            run_id: run.id.clone(),
            task: "task1".to_string(),
            status: TaskStatus::Pending,
            attempt: 0,
            max_retries: 1,
            created_at: chrono::Utc::now(),
            dispatched_at: None,
            started_at: None,
            finished_at: None,
            error: None,
            output: None,
        };
        store
            .upsert_task_run(task_run)
            .await
            .expect("upsert task run");

        let tasks = store.list_task_runs(&run.id).await.expect("list task runs");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task, "task1");
    }

    #[tokio::test]
    async fn test_update_run_status_sets_timestamps() {
        let store = InMemoryStateStore::default();
        let workflow = sample_workflow();
        let run = store.create_run(&workflow).await.expect("create run");

        store
            .update_run_status(&run.id, RunStatus::Running)
            .await
            .expect("set running");
        store
            .update_run_status(&run.id, RunStatus::Success)
            .await
            .expect("set success");

        let updated = store
            .get_run(&run.id)
            .await
            .expect("get run")
            .expect("run exists");
        assert_eq!(updated.status, RunStatus::Success);
        assert!(updated.started_at.is_some());
        assert!(updated.finished_at.is_some());
    }
}
