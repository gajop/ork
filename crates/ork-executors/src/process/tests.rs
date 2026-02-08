use super::*;
use std::fs;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[test]
fn test_build_env_vars_extracts_known_fields() {
    let params = serde_json::json!({
        "task_file": "tasks/job.py",
        "task_input": {"value": 42},
        "task_module": "pkg.tasks",
        "task_function": "run",
        "python_path": "/tmp/python-root",
        "runner_path": "/tmp/runner.py",
        "upstream": {"step_a": {"ok": true}},
        "string_meta": "x",
        "numeric_meta": 7,
        "bool_meta": true
    });

    let (env_vars, task_file, task_module, task_function, task_input, runner_path, python_path) =
        build_env_vars(Some(params));

    assert_eq!(task_file.expect("task_file"), PathBuf::from("tasks/job.py"));
    assert_eq!(task_module.as_deref(), Some("pkg.tasks"));
    assert_eq!(task_function.as_deref(), Some("run"));
    assert_eq!(
        python_path.expect("python_path"),
        PathBuf::from("/tmp/python-root")
    );
    assert_eq!(
        runner_path.expect("runner_path"),
        PathBuf::from("/tmp/runner.py")
    );
    assert_eq!(
        task_input.expect("task_input"),
        serde_json::json!({"value": 42})
    );

    assert_eq!(env_vars.get("string_meta").map(String::as_str), Some("x"));
    assert_eq!(env_vars.get("numeric_meta").map(String::as_str), Some("7"));
    assert_eq!(env_vars.get("bool_meta").map(String::as_str), Some("true"));
    assert!(env_vars.contains_key("ORK_INPUT_JSON"));
    assert!(env_vars.contains_key("ORK_UPSTREAM_JSON"));
}

#[test]
fn test_build_env_vars_with_none_returns_defaults() {
    let (env_vars, task_file, task_module, task_function, task_input, runner_path, python_path) =
        build_env_vars(None);
    assert!(env_vars.is_empty());
    assert!(task_file.is_none());
    assert!(task_module.is_none());
    assert!(task_function.is_none());
    assert!(task_input.is_none());
    assert!(runner_path.is_none());
    assert!(python_path.is_none());
}

#[test]
fn test_resolve_command_path_behavior() {
    assert_eq!(
        resolve_command_path(".", "/usr/bin/echo"),
        "/usr/bin/echo".to_string()
    );
    assert_eq!(
        resolve_command_path(".", "echo hello"),
        "echo hello".to_string()
    );
    assert_eq!(
        resolve_command_path(".", "scripts/run.sh"),
        "scripts/run.sh".to_string()
    );
    assert_eq!(
        resolve_command_path(".", "my-task"),
        "./my-task".to_string()
    );
}

#[test]
fn test_find_project_root_prefers_pyproject_ancestor() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    let nested = base.join("a").join("b");
    fs::create_dir_all(&nested).expect("create nested dir");
    fs::write(base.join("pyproject.toml"), "[project]\nname='x'\n").expect("write pyproject");
    let task_path = nested.join("task.py");
    fs::write(&task_path, "print('ok')\n").expect("write task");

    let root = find_project_root(&task_path).expect("project root");
    assert_eq!(root, base);

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_find_project_root_falls_back_to_parent() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    let nested = base.join("x").join("y");
    fs::create_dir_all(&nested).expect("create nested dir");
    let task_path = nested.join("task.py");
    fs::write(&task_path, "print('ok')\n").expect("write task");

    let root = find_project_root(&task_path).expect("project root");
    assert_eq!(root, nested);

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_build_python_command_uses_uv_when_pyproject_exists() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    let nested = base.join("src");
    fs::create_dir_all(&nested).expect("create nested dir");
    fs::write(base.join("pyproject.toml"), "[project]\nname='x'\n").expect("write pyproject");
    let task_path = nested.join("task.py");
    fs::write(&task_path, "print('ok')\n").expect("write task");

    let (cmd, uses_uv) = build_python_command(&task_path, None);
    assert!(uses_uv);
    assert_eq!(cmd.as_std().get_program().to_string_lossy(), "uv");

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn test_build_python_command_falls_back_to_python3() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create dir");
    let task_path = base.join("task.py");
    fs::write(&task_path, "print('ok')\n").expect("write task");

    let (cmd, uses_uv) = build_python_command(&task_path, None);
    assert!(!uses_uv);
    assert_eq!(cmd.as_std().get_program().to_string_lossy(), "python3");

    let _ = fs::remove_dir_all(&base);
}

async fn collect_updates(
    rx: &mut mpsc::UnboundedReceiver<StatusUpdate>,
) -> anyhow::Result<Vec<StatusUpdate>> {
    let mut updates = Vec::new();
    timeout(Duration::from_secs(5), async {
        loop {
            let update = rx.recv().await.expect("status update");
            let terminal = update.status == "success" || update.status == "failed";
            updates.push(update);
            if terminal {
                break;
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for process updates"))?;
    Ok(updates)
}

#[tokio::test]
async fn test_execute_process_reports_success_logs_and_output() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(
            task_id,
            "printf 'line\\n'; printf 'ORK_OUTPUT:{\"ok\":true}\\n'; printf 'warn\\n' 1>&2",
            Some(serde_json::json!({"task_name": "demo"})),
        )
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    assert!(
        updates
            .iter()
            .any(|u| u.status == "running" && u.task_id == task_id)
    );
    assert!(
        updates
            .iter()
            .any(|u| u.status == "log" && u.log.as_deref().unwrap_or("").contains("stdout: line"))
    );
    assert!(
        updates
            .iter()
            .any(|u| u.status == "log" && u.log.as_deref().unwrap_or("").contains("stderr: warn"))
    );
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert_eq!(terminal.output, Some(serde_json::json!({"ok": true})));
    assert!(terminal.error.is_none());

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Success));
}

#[tokio::test]
async fn test_execute_process_reports_failed_status() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "exit 3", None)
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(terminal.output.is_none());
    assert!(
        terminal
            .error
            .as_deref()
            .unwrap_or("")
            .contains("process failed for task")
    );

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Failed));
}

#[tokio::test]
async fn test_execute_process_spawn_failure_emits_terminal_failure() {
    let missing_dir = std::env::temp_dir().join(format!("ork-missing-{}", Uuid::new_v4()));
    let executor = ProcessExecutor::new(Some(missing_dir.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "echo should-not-run", None)
        .await
        .expect("execute process should still return execution id");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(
        terminal
            .error
            .as_deref()
            .unwrap_or("")
            .contains("failed to spawn process")
    );

    let states = executor.process_states.read().await;
    let state = states.get(&execution_id).expect("process state");
    assert!(matches!(state, ProcessStatus::Failed));
}

#[tokio::test]
async fn test_execute_process_with_task_module_runner_path() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");

    let runner = base.join("runner.py");
    fs::write(
        &runner,
        r#"
import json
import os

payload = {
    "module": os.environ.get("ORK_TASK_MODULE"),
    "function": os.environ.get("ORK_TASK_FUNCTION"),
    "workflow": os.environ.get("ORK_WORKFLOW_NAME"),
    "input": json.loads(os.environ.get("ORK_INPUT_JSON", "{}")),
}
print("ORK_OUTPUT:" + json.dumps(payload))
"#,
    )
    .expect("write runner");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_module": "pkg.tasks",
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_input": {"value": 9},
                "task_name": "demo-task",
                "workflow_name": "demo-workflow",
                "run_id": task_id.to_string(),
            })),
        )
        .await
        .expect("execute process with module");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert_eq!(output["module"], "pkg.tasks");
    assert_eq!(output["function"], "run");
    assert_eq!(output["workflow"], "demo-workflow");
    assert_eq!(output["input"], serde_json::json!({"value": 9}));

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_success_without_output_payload() {
    let executor = ProcessExecutor::new(None);
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute_process(task_id, "printf 'just-logs\\n'", None)
        .await
        .expect("execute process");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    assert!(terminal.output.is_none());
    assert!(terminal.error.is_none());
}

#[tokio::test]
async fn test_execute_process_with_task_file_sets_expected_env() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");

    let task_file = base.join("task.py");
    fs::write(&task_file, "def run(x: int) -> int:\n    return x\n").expect("write task file");

    let runner = base.join("runner.py");
    fs::write(
        &runner,
        r#"
import json
import os

print(
    "ORK_OUTPUT:"
    + json.dumps(
        {
            "task_file": os.environ.get("ORK_TASK_FILE"),
            "task_function": os.environ.get("ORK_TASK_FUNCTION"),
            "task_name": os.environ.get("ORK_TASK_NAME"),
            "workflow_name": os.environ.get("ORK_WORKFLOW_NAME"),
            "run_id": os.environ.get("ORK_RUN_ID"),
            "pythonpath": os.environ.get("PYTHONPATH"),
            "input": json.loads(os.environ.get("ORK_INPUT_JSON", "{}")),
        }
    )
)
"#,
    )
    .expect("write runner");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "task_function": "run",
                "runner_path": runner,
                "python_path": base,
                "task_input": {"value": 123},
                "task_name": "task-a",
                "workflow_name": "wf-a",
                "run_id": task_id.to_string(),
            })),
        )
        .await
        .expect("execute process with task file");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "success");
    let output = terminal.output.clone().expect("output payload");
    assert!(
        output["task_file"]
            .as_str()
            .unwrap_or("")
            .ends_with("task.py")
    );
    assert_eq!(output["task_function"], "run");
    assert_eq!(output["task_name"], "task-a");
    assert_eq!(output["workflow_name"], "wf-a");
    assert_eq!(output["run_id"], task_id.to_string());
    assert_eq!(output["input"], serde_json::json!({"value": 123}));
    assert_eq!(
        output["pythonpath"].as_str().unwrap_or(""),
        base.to_string_lossy()
    );

    let _ = fs::remove_dir_all(&base);
}

#[tokio::test]
async fn test_execute_process_with_uv_path_sets_cache_env_and_fails_cleanly() {
    let base = std::env::temp_dir().join(format!("ork-process-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&base).expect("create temp dir");
    fs::write(base.join("pyproject.toml"), "[project]\nname='x'\n").expect("write pyproject");

    let task_file = base.join("task.py");
    fs::write(&task_file, "def run(x: int) -> int:\n    return x\n").expect("write task file");
    let missing_runner = base.join("missing-runner.py");

    let executor = ProcessExecutor::new(Some(base.to_string_lossy().to_string()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    executor.set_status_channel(tx).await;
    let task_id = Uuid::new_v4();

    let execution_id = executor
        .execute(
            task_id,
            "ignored",
            Some(serde_json::json!({
                "task_file": task_file,
                "runner_path": missing_runner,
                "python_path": base,
                "task_input": {"value": 1},
                "task_name": "uv-task",
            })),
        )
        .await
        .expect("execute process with uv path");
    assert!(!execution_id.is_empty());

    let updates = collect_updates(&mut rx).await.expect("collect updates");
    let terminal = updates.last().expect("terminal update");
    assert_eq!(terminal.status, "failed");
    assert!(terminal.output.is_none());

    let _ = fs::remove_dir_all(&base);
}
