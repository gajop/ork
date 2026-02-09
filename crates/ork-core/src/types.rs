use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::compiled::CompiledWorkflow;
use crate::workflow::{ExecutorKind, Workflow};

pub type RunId = String;
pub type TaskName = String;
pub type WorkflowName = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    Success,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Paused,
    Dispatched,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub workflow: WorkflowName,
    pub status: RunStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub run_id: RunId,
    pub task: TaskName,
    pub status: TaskStatus,
    pub attempt: u32,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub output: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub run_id: RunId,
    pub workflow_name: WorkflowName,
    pub task_name: TaskName,
    pub attempt: u32,
    pub executor: ExecutorKind,
    pub input: Value,
    #[serde(default)]
    pub upstream: IndexMap<TaskName, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusFile {
    pub status: TaskStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl TaskSpec {
    pub fn from_workflow(
        workflow: &Workflow,
        run_id: RunId,
        task_name: &str,
        attempt: u32,
        upstream: IndexMap<TaskName, Value>,
    ) -> Option<Self> {
        let definition = workflow.tasks.get(task_name)?;
        Some(TaskSpec {
            run_id,
            workflow_name: workflow.name.clone(),
            task_name: task_name.to_string(),
            attempt,
            executor: definition.executor.clone(),
            input: definition.input.clone(),
            upstream,
        })
    }
}

impl TaskSpec {
    pub fn from_compiled(
        workflow: &CompiledWorkflow,
        run_id: RunId,
        task_idx: usize,
        attempt: u32,
        upstream: IndexMap<TaskName, Value>,
    ) -> Option<Self> {
        let definition = workflow.tasks.get(task_idx)?;
        Some(TaskSpec {
            run_id,
            workflow_name: workflow.name.clone(),
            task_name: definition.name.clone(),
            attempt,
            executor: definition.executor.clone(),
            input: definition.input.clone(),
            upstream,
        })
    }
}

impl Run {
    pub fn new(workflow: &Workflow) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            workflow: workflow.name.clone(),
            status: RunStatus::Pending,
            created_at: now,
            started_at: None,
            finished_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::{CompiledTask, CompiledWorkflow};
    use crate::workflow::TaskDefinition;
    use std::path::PathBuf;

    fn sample_workflow() -> Workflow {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "task_a".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo ok".to_string()),
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({"x": 1}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 1,
                input_type: None,
                output_type: None,
            },
        );
        Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        }
    }

    #[test]
    fn test_task_spec_from_workflow_and_missing_task() {
        let workflow = sample_workflow();
        let upstream = IndexMap::from([(String::from("dep"), serde_json::json!({"ok": true}))]);
        let spec = TaskSpec::from_workflow(&workflow, "run-1".to_string(), "task_a", 2, upstream)
            .expect("task should exist");

        assert_eq!(spec.workflow_name, "wf");
        assert_eq!(spec.task_name, "task_a");
        assert_eq!(spec.attempt, 2);
        assert!(matches!(spec.executor, ExecutorKind::Process));
        assert_eq!(spec.input, serde_json::json!({"x": 1}));
        assert!(
            TaskSpec::from_workflow(
                &workflow,
                "run-1".to_string(),
                "missing",
                1,
                IndexMap::new()
            )
            .is_none()
        );
    }

    #[test]
    fn test_task_spec_from_compiled_and_missing_index() {
        let compiled = CompiledWorkflow {
            name: "wf-compiled".to_string(),
            tasks: vec![CompiledTask {
                name: "task0".to_string(),
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo".to_string()),
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({"value": 42}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 0,
                signature: None,
                input_type: None,
                output_type: None,
            }],
            name_index: IndexMap::from([(String::from("task0"), 0)]),
            topo: vec![0],
            root: PathBuf::from("."),
        };

        let spec = TaskSpec::from_compiled(&compiled, "run-2".to_string(), 0, 1, IndexMap::new())
            .expect("compiled task should exist");
        assert_eq!(spec.workflow_name, "wf-compiled");
        assert_eq!(spec.task_name, "task0");
        assert_eq!(spec.input, serde_json::json!({"value": 42}));
        assert!(
            TaskSpec::from_compiled(&compiled, "run-2".to_string(), 9, 1, IndexMap::new())
                .is_none()
        );
    }

    #[test]
    fn test_run_new_initializes_pending_state() {
        let workflow = sample_workflow();
        let run = Run::new(&workflow);
        assert_eq!(run.workflow, workflow.name);
        assert_eq!(run.status, RunStatus::Pending);
        assert!(run.started_at.is_none());
        assert!(run.finished_at.is_none());
        assert!(!run.id.is_empty());
    }
}
