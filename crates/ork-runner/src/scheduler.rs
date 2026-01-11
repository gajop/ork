use std::path::{Path, PathBuf};
use std::sync::Arc;

use ork_core::compiled::CompiledWorkflow;
use ork_core::error::OrkResult;
use ork_core::types::{Run, RunStatus, TaskRun};
use ork_core::workflow::Workflow;
use ork_state::{ObjectStore, StateStore};

use crate::runner::{RunSummary, run_workflow};

#[derive(Clone)]
pub struct LocalScheduler {
    pub(crate) state: Arc<dyn StateStore>,
    pub(crate) object_store: Arc<dyn ObjectStore>,
    pub(crate) base_dir: PathBuf,
    pub(crate) max_parallel: usize,
}

impl LocalScheduler {
    pub fn with_state(
        state: Arc<dyn StateStore>,
        object_store: Arc<dyn ObjectStore>,
        base_dir: impl AsRef<Path>,
        max_parallel: usize,
    ) -> Self {
        Self {
            state,
            object_store,
            base_dir: base_dir.as_ref().to_path_buf(),
            max_parallel,
        }
    }

    pub async fn run_compiled(
        &self,
        workflow: Workflow,
        compiled: CompiledWorkflow,
    ) -> OrkResult<RunSummary> {
        // Fail fast if validation/compile didn't succeed earlier
        let run = self.state.create_run(&workflow).await?;
        let summary = run_workflow(
            &compiled,
            &self.base_dir,
            self.max_parallel,
            Some(run.id.clone()),
            self.state.clone(),
            self.object_store.clone(),
        )
        .await?;

        let run_status = summarize_run(&summary);
        self.state.update_run_status(&run.id, run_status).await?;

        Ok(summary)
    }

    pub async fn get_run(&self, run_id: &str) -> OrkResult<Option<Run>> {
        self.state.get_run(&run_id.to_string()).await
    }

    pub async fn task_runs(&self, run_id: &str) -> OrkResult<Vec<TaskRun>> {
        self.state.list_task_runs(&run_id.to_string()).await
    }

    pub async fn list_runs(&self) -> OrkResult<Vec<Run>> {
        self.state.list_runs().await
    }
}

fn summarize_run(summary: &RunSummary) -> RunStatus {
    let mut any_failed = false;
    for status in summary.statuses.values() {
        match status {
            crate::runner::LocalTaskState::Failed { .. } => any_failed = true,
            crate::runner::LocalTaskState::Skipped { .. } => any_failed = true,
            _ => {}
        }
    }

    if any_failed {
        RunStatus::Failed
    } else {
        RunStatus::Success
    }
}
