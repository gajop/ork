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
