use super::*;
use crate::executor::Executor;
use crate::executor_manager::ExecutorManager;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, mpsc};

fn sample_task_with_workflow(params: Option<serde_json::Value>) -> TaskWithWorkflow {
    TaskWithWorkflow {
        task_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
        task_index: 0,
        task_name: "task".to_string(),
        executor_type: "process".to_string(),
        depends_on: Vec::new(),
        task_status: TaskStatus::Pending,
        attempts: 0,
        max_retries: 0,
        timeout_seconds: None,
        retry_at: None,
        execution_name: None,
        params: params.map(Into::into),
        workflow_id: Uuid::new_v4(),
        job_name: "workflow-job".to_string(),
        project: "project".to_string(),
        region: "region".to_string(),
    }
}

fn sample_workflow(id: Uuid) -> Workflow {
    Workflow {
        id,
        name: "wf".to_string(),
        description: None,
        job_name: "job".to_string(),
        region: "local".to_string(),
        project: "local".to_string(),
        executor_type: "process".to_string(),
        task_params: None,
        schedule: None,
        schedule_enabled: false,
        last_scheduled_at: None,
        next_scheduled_at: None,
        current_snapshot_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

struct RecordingExecutor {
    captured: Mutex<Option<(Uuid, String, Option<serde_json::Value>)>>,
    fail_execute: bool,
    status_channel_set: AtomicBool,
}

impl RecordingExecutor {
    fn new(fail_execute: bool) -> Self {
        Self {
            captured: Mutex::new(None),
            fail_execute,
            status_channel_set: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl Executor for RecordingExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> anyhow::Result<String> {
        *self.captured.lock().await = Some((task_id, job_name.to_string(), params));
        if self.fail_execute {
            Err(anyhow::anyhow!("mock execute failure"))
        } else {
            Ok("exec-123".to_string())
        }
    }

    async fn set_status_channel(&self, _tx: mpsc::UnboundedSender<StatusUpdate>) {
        self.status_channel_set.store(true, Ordering::SeqCst);
    }
}

struct RecordingManager {
    executor: Arc<RecordingExecutor>,
    fail_get_executor: bool,
}

#[async_trait]
impl ExecutorManager for RecordingManager {
    async fn get_executor(
        &self,
        _executor_type: &str,
        _workflow: &Workflow,
    ) -> anyhow::Result<Arc<dyn Executor>> {
        if self.fail_get_executor {
            Err(anyhow::anyhow!("mock get_executor failure"))
        } else {
            Ok(self.executor.clone())
        }
    }
}

#[test]
fn test_resolve_job_name_prefers_params_over_task_default() {
    let with_command = sample_task_with_workflow(Some(serde_json::json!({
        "command": "echo hello"
    })));
    assert_eq!(resolve_job_name(&with_command), "echo hello");

    let with_job_name = sample_task_with_workflow(Some(serde_json::json!({
        "job_name": "overridden"
    })));
    assert_eq!(resolve_job_name(&with_job_name), "overridden");
}

#[test]
fn test_resolve_job_name_falls_back_to_task_joined_data() {
    let task = sample_task_with_workflow(None);
    assert_eq!(resolve_job_name(&task), "workflow-job");
}

#[test]
fn test_retry_backoff_seconds_caps() {
    assert_eq!(retry_backoff_seconds(0), 1);
    assert_eq!(retry_backoff_seconds(1), 1);
    assert_eq!(retry_backoff_seconds(2), 2);
    assert_eq!(retry_backoff_seconds(6), 32);
    assert_eq!(retry_backoff_seconds(10), 60);
    assert_eq!(retry_backoff_seconds(15), 60);
}

#[tokio::test]
async fn test_execute_task_rejects_non_pending_status() {
    let mut task = sample_task_with_workflow(None);
    task.task_status = TaskStatus::Running;

    let workflow_map = Arc::new(HashMap::from([(
        task.workflow_id,
        sample_workflow(task.workflow_id),
    )]));
    let outputs_by_run = Arc::new(HashMap::new());
    let manager = RecordingManager {
        executor: Arc::new(RecordingExecutor::new(false)),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    let err = result.expect_err("non-pending task must fail");
    assert!(err.to_string().contains("non-pending status"));
}

#[tokio::test]
async fn test_execute_task_errors_when_workflow_missing() {
    let task = sample_task_with_workflow(None);
    let workflow_map = Arc::new(HashMap::new());
    let outputs_by_run = Arc::new(HashMap::new());
    let manager = RecordingManager {
        executor: Arc::new(RecordingExecutor::new(false)),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    let err = result.expect_err("missing workflow must fail");
    assert!(err.to_string().contains("not found for task"));
}

#[tokio::test]
async fn test_execute_task_builds_attempt_env_without_implicit_upstream() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "env": "bad-type",
        "task_input": {}
    })));
    task.attempts = 1;
    task.depends_on = vec!["dep_a".to_string(), "dep_missing".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep_a".to_string(), serde_json::json!({"v": 1}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    assert_eq!(result.expect("execute result"), "exec-123");
    assert!(executor.status_channel_set.load(Ordering::SeqCst));

    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params should be present");
    assert_eq!(params["env"]["ORK_ATTEMPT"], 2);
    assert_eq!(params["task_input"], serde_json::json!({}));
    assert!(params.get("upstream").is_none());
}

#[tokio::test]
async fn test_execute_task_preserves_non_empty_task_input() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_input": {"keep": true}
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), serde_json::json!(5))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    assert!(result.is_ok());
    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params");
    assert_eq!(params["task_input"]["keep"], true);
    assert!(params.get("upstream").is_none());
}

#[tokio::test]
async fn test_execute_task_normalizes_non_object_params_without_injecting_inputs() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!("bad-params")));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), serde_json::json!(7))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    assert!(result.is_ok());
    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params");
    assert_eq!(params["env"]["ORK_ATTEMPT"], 1);
    assert!(params.get("task_input").is_none());
    assert!(params.get("upstream").is_none());
}

#[tokio::test]
async fn test_execute_task_preserves_null_task_input_without_upstream_injection() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_input": null
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), serde_json::json!({"k": "v"}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    assert!(result.is_ok());
    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params");
    assert!(params["task_input"].is_null());
    assert!(params.get("upstream").is_none());
}

#[tokio::test]
async fn test_execute_task_returns_executor_manager_error() {
    let task = sample_task_with_workflow(None);
    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
    let manager = RecordingManager {
        executor: Arc::new(RecordingExecutor::new(false)),
        fail_get_executor: true,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    let err = result.expect_err("get_executor must fail");
    assert!(err.to_string().contains("mock get_executor failure"));
}

#[tokio::test]
async fn test_execute_task_resolves_task_bindings_into_task_input() {
    let mut task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "from_const": {"const": 15},
            "from_ref": {"ref": "tasks.dep.output.x"}
        }
    })));
    task.depends_on = vec!["dep".to_string()];

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::from([(
        task.run_id,
        HashMap::from([("dep".to_string(), serde_json::json!({"x": 7}))]),
    )]));
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    assert!(result.is_ok());

    let (_id, _job_name, params) = executor
        .captured
        .lock()
        .await
        .clone()
        .expect("captured execute params");
    let params = params.expect("params should be present");
    assert_eq!(params["task_input"]["from_const"], 15);
    assert_eq!(params["task_input"]["from_ref"], 7);
    assert_eq!(params.get("upstream"), None);
}

#[tokio::test]
async fn test_execute_task_binding_ref_requires_dependency() {
    let task = sample_task_with_workflow(Some(serde_json::json!({
        "task_bindings": {
            "x": {"ref": "tasks.other.output.value"}
        }
    })));

    let workflow = sample_workflow(task.workflow_id);
    let workflow_map = Arc::new(HashMap::from([(workflow.id, workflow)]));
    let outputs_by_run = Arc::new(HashMap::new());
    let executor = Arc::new(RecordingExecutor::new(false));
    let manager = RecordingManager {
        executor: executor.clone(),
        fail_get_executor: false,
    };
    let (tx, _rx) = mpsc::unbounded_channel();

    let (_task_id, result) = execute_task(task, workflow_map, outputs_by_run, &manager, tx).await;
    let err = result.expect_err("binding ref without dependency should fail");
    assert!(
        err.to_string().contains("not listed in depends_on"),
        "unexpected error: {err}"
    );
    assert!(
        executor.captured.lock().await.is_none(),
        "executor should not be called on binding resolution error"
    );
}

#[test]
fn test_build_run_tasks_normalizes_params_and_clamps_limits() {
    let run_id = Uuid::new_v4();
    let workflow_id = Uuid::new_v4();
    let workflow = sample_workflow(workflow_id);
    let now = Utc::now();
    let workflow_tasks = vec![
        WorkflowTask {
            id: Uuid::new_v4(),
            workflow_id,
            task_index: 0,
            task_name: "a".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec![],
            params: Some(serde_json::json!("bad").into()),
            created_at: now,
        },
        WorkflowTask {
            id: Uuid::new_v4(),
            workflow_id,
            task_index: 1,
            task_name: "b".to_string(),
            executor_type: "process".to_string(),
            depends_on: vec!["a".to_string()],
            params: Some(
                serde_json::json!({
                    "max_retries": i64::MAX,
                    "timeout_seconds": -20,
                    "task_name": "custom-name"
                })
                .into(),
            ),
            created_at: now,
        },
    ];

    let built = build_run_tasks(run_id, &workflow, &workflow_tasks);
    assert_eq!(built.len(), 2);

    assert_eq!(built[0].max_retries, 0);
    assert_eq!(built[0].timeout_seconds, None);
    assert_eq!(built[0].params["task_index"], 0);
    assert_eq!(built[0].params["task_name"], "a");
    assert_eq!(built[0].params["workflow_name"], "wf");
    assert_eq!(built[0].params["run_id"], run_id.to_string());

    assert_eq!(built[1].max_retries, i32::MAX);
    assert_eq!(built[1].timeout_seconds, Some(0));
    assert_eq!(built[1].params["task_name"], "custom-name");
}
