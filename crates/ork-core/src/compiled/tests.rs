use super::*;
use crate::workflow::{ExecutorKind, Workflow};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

fn write_file(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).expect("write file");
    path
}

fn script_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct ScriptRestore {
    path: std::path::PathBuf,
    original: String,
}

impl Drop for ScriptRestore {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.path, &self.original);
    }
}

#[test]
fn test_compile_python_module_without_file_skips_signature_introspection() {
    let dir = TempDir::new().expect("temp dir");
    let yaml = r#"
name: python-module-only
tasks:
  only:
    executor: python
    module: app.tasks
"#;
    let workflow: Workflow = serde_yaml::from_str(yaml).expect("workflow yaml");
    let compiled = workflow.compile(dir.path()).expect("compile");
    let idx = *compiled.name_index.get("only").expect("task index");
    let task = &compiled.tasks[idx];

    assert!(task.file.is_none());
    assert!(task.signature.is_none());
}

#[test]
fn test_get_inspect_script_is_cached_on_second_call() {
    let _guard = script_lock().lock().expect("script lock");
    let first = get_inspect_script().expect("first inspect script path");
    assert!(first.exists(), "inspect script should be written");

    let second = get_inspect_script().expect("second inspect script path");
    assert_eq!(first, second, "inspect script path should be stable");
}

#[test]
fn test_get_inspect_script_recreates_missing_script_file() {
    let _guard = script_lock().lock().expect("script lock");
    let path = get_inspect_script().expect("inspect script path");
    if path.exists() {
        std::fs::remove_file(&path).expect("remove inspect script");
    }
    assert!(
        !path.exists(),
        "inspect script should be removed for this test"
    );

    let recreated = get_inspect_script().expect("recreate inspect script");
    assert_eq!(recreated, path);
    assert!(recreated.exists(), "inspect script should be recreated");
}

#[test]
fn test_introspect_python_signature_surfaces_python_side_error() {
    let _guard = script_lock().lock().expect("script lock");
    let dir = TempDir::new().expect("temp dir");
    let file = write_file(
        &dir,
        "task.py",
        r#"
def actual_task(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature(&file, "missing_task")
        .expect_err("missing function should fail introspection");
    assert!(
        err.contains("Task analysis error") || err.contains("Failed to parse introspection output"),
        "expected introspection failure, got: {err}"
    );
}

#[test]
fn test_introspect_python_signature_reports_non_json_success_output() {
    let _guard = script_lock().lock().expect("script lock");
    let script = get_inspect_script().expect("inspect script path");
    let original = std::fs::read_to_string(&script).expect("read script");
    let _restore = ScriptRestore {
        path: script.clone(),
        original,
    };
    std::fs::write(
        &script,
        r#"
import sys
print("not-json")
sys.exit(0)
"#,
    )
    .expect("overwrite script");

    let dir = TempDir::new().expect("temp dir");
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature(&file, "main")
        .expect_err("non-json stdout should fail parsing");
    assert!(err.contains("Failed to parse introspection output"));
}

#[test]
fn test_introspect_python_signature_success_path() {
    let _guard = script_lock().lock().expect("script lock");
    let dir = TempDir::new().expect("temp dir");
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int, y: str) -> bool:
    return True
"#,
    );

    let value = introspect_python_signature(&file, "main").expect("successful introspection");
    assert_eq!(value["inputs"]["x"], "int");
    assert_eq!(value["inputs"]["y"], "str");
    assert_eq!(value["output"], "bool");
}

#[test]
fn test_introspect_python_signature_reports_script_runtime_failure() {
    let _guard = script_lock().lock().expect("script lock");
    let script = get_inspect_script().expect("inspect script path");
    let original = std::fs::read_to_string(&script).expect("read script");
    let _restore = ScriptRestore {
        path: script.clone(),
        original,
    };
    std::fs::write(
        &script,
        r#"
raise RuntimeError("broken inspect script")
"#,
    )
    .expect("overwrite script");

    let dir = TempDir::new().expect("temp dir");
    let file = write_file(
        &dir,
        "task.py",
        r#"
def main(x: int) -> int:
    return x + 1
"#,
    );

    let err = introspect_python_signature(&file, "main")
        .expect_err("runtime failure should return stderr-based error");
    assert!(err.contains("Introspection failed"));
}

#[test]
fn test_topo_sort_returns_cycle_error_when_graph_is_cyclic() {
    let tasks = vec![
        CompiledTask {
            name: "a".to_string(),
            executor: crate::workflow::ExecutorKind::Process,
            file: None,
            command: Some("echo a".to_string()),
            job: None,
            module: None,
            function: None,
            input: serde_json::json!({}),
            depends_on: vec![1],
            timeout: 300,
            retries: 0,
            signature: None,
            input_type: None,
            output_type: None,
        },
        CompiledTask {
            name: "b".to_string(),
            executor: crate::workflow::ExecutorKind::Process,
            file: None,
            command: Some("echo b".to_string()),
            job: None,
            module: None,
            function: None,
            input: serde_json::json!({}),
            depends_on: vec![0],
            timeout: 300,
            retries: 0,
            signature: None,
            input_type: None,
            output_type: None,
        },
    ];

    let err = topo_sort(&tasks).expect_err("cyclic graph should fail topo sort");
    assert!(matches!(
        err,
        crate::error::OrkError::InvalidWorkflow(
            crate::error::WorkflowValidationError::Cycle { .. }
        )
    ));
}

// Tests for build_workflow_tasks

fn compiled_fixture() -> CompiledWorkflow {
    CompiledWorkflow {
        name: "fixture".to_string(),
        tasks: vec![
            CompiledTask {
                name: "process_task".to_string(),
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo hello".to_string()),
                job: None,
                module: None,
                function: None,
                input: serde_json::Value::Null,
                depends_on: vec![],
                timeout: 30,
                retries: 2,
                signature: None,
                input_type: None,
                output_type: None,
            },
            CompiledTask {
                name: "python_task".to_string(),
                executor: ExecutorKind::Python,
                file: Some(PathBuf::from("tasks/example.py")),
                command: None,
                job: None,
                module: Some("tasks.example".to_string()),
                function: Some("run".to_string()),
                input: serde_json::json!({"k": "v"}),
                depends_on: vec![0],
                timeout: 45,
                retries: 1,
                signature: Some(serde_json::json!({"args": []})),
                input_type: None,
                output_type: None,
            },
            CompiledTask {
                name: "cloud_task".to_string(),
                executor: ExecutorKind::CloudRun,
                file: None,
                command: None,
                job: Some("cloud-job".to_string()),
                module: None,
                function: None,
                input: serde_json::Value::Null,
                depends_on: vec![0, 1],
                timeout: 60,
                retries: 0,
                signature: None,
                input_type: None,
                output_type: None,
            },
            CompiledTask {
                name: "library_task".to_string(),
                executor: ExecutorKind::Library,
                file: Some(PathBuf::from("target/libtask.so")),
                command: None,
                job: None,
                module: None,
                function: None,
                input: serde_json::Value::Null,
                depends_on: vec![2],
                timeout: 15,
                retries: 0,
                signature: None,
                input_type: None,
                output_type: None,
            },
        ],
        name_index: Default::default(),
        topo: vec![0, 1, 2, 3],
        root: PathBuf::from("/tmp/ork-workflow"),
    }
}

#[test]
fn test_build_workflow_tasks_maps_dependencies_and_executor_types() {
    let compiled = compiled_fixture();
    let tasks = build_workflow_tasks(&compiled);

    assert_eq!(tasks.len(), 4);
    assert_eq!(tasks[0].executor_type, "process");
    assert_eq!(tasks[1].executor_type, "python");
    assert_eq!(tasks[2].executor_type, "cloudrun");
    assert_eq!(tasks[3].executor_type, "library");

    assert_eq!(tasks[0].depends_on, Vec::<String>::new());
    assert_eq!(tasks[1].depends_on, vec!["process_task".to_string()]);
    assert_eq!(
        tasks[2].depends_on,
        vec!["process_task".to_string(), "python_task".to_string()]
    );
    assert_eq!(tasks[3].depends_on, vec!["cloud_task".to_string()]);
}

#[test]
fn test_build_workflow_tasks_populates_executor_specific_params() {
    let compiled = compiled_fixture();
    let tasks = build_workflow_tasks(&compiled);

    let process_params = &tasks[0].params;
    assert_eq!(
        process_params.get("command"),
        Some(&serde_json::json!("echo hello"))
    );
    assert_eq!(
        process_params.get("max_retries"),
        Some(&serde_json::json!(2))
    );
    assert_eq!(
        process_params.get("timeout_seconds"),
        Some(&serde_json::json!(30))
    );

    let python_params = &tasks[1].params;
    assert_eq!(
        python_params.get("task_file"),
        Some(&serde_json::json!("tasks/example.py"))
    );
    assert_eq!(
        python_params.get("task_module"),
        Some(&serde_json::json!("tasks.example"))
    );
    assert_eq!(
        python_params.get("task_function"),
        Some(&serde_json::json!("run"))
    );
    assert_eq!(
        python_params.get("python_path"),
        Some(&serde_json::json!("/tmp/ork-workflow"))
    );
    assert_eq!(
        python_params.get("task_input"),
        Some(&serde_json::json!({"k": "v"}))
    );

    let cloud_params = &tasks[2].params;
    assert_eq!(
        cloud_params.get("job_name"),
        Some(&serde_json::json!("cloud-job"))
    );

    let library_params = &tasks[3].params;
    assert_eq!(
        library_params.get("library_path"),
        Some(&serde_json::json!("target/libtask.so"))
    );
}

#[test]
fn test_build_workflow_tasks_resolves_relative_process_command_from_workflow_root() {
    let mut compiled = compiled_fixture();
    compiled.tasks[0].command = Some("bin/process-task".to_string());
    compiled.root = PathBuf::from("/tmp/ork-workflow");

    let tasks = build_workflow_tasks(&compiled);

    assert_eq!(
        tasks[0].params.get("command"),
        Some(&serde_json::json!("/tmp/ork-workflow/bin/process-task"))
    );
}

#[test]
fn test_build_workflow_tasks_maps_process_file_into_command() {
    let compiled = CompiledWorkflow {
        name: "wf".to_string(),
        tasks: vec![CompiledTask {
            name: "from-file".to_string(),
            executor: ExecutorKind::Process,
            file: Some(PathBuf::from("/tmp/task.py")),
            command: None,
            job: None,
            module: None,
            function: None,
            input: serde_json::Value::Null,
            depends_on: vec![],
            timeout: 5,
            retries: 0,
            signature: None,
            input_type: None,
            output_type: None,
        }],
        name_index: indexmap::IndexMap::new(),
        topo: vec![0],
        root: PathBuf::from("/tmp"),
    };

    let tasks = build_workflow_tasks(&compiled);
    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].params["command"],
        serde_json::json!("/tmp/task.py")
    );
}

#[test]
fn test_resolve_process_command_keeps_commands_with_whitespace() {
    let root = Path::new("/tmp");
    assert_eq!(
        resolve_process_command("echo hello world", root),
        "echo hello world"
    );
}

#[test]
fn test_resolve_process_command_keeps_absolute_paths() {
    let root = Path::new("/tmp");
    assert_eq!(
        resolve_process_command("/usr/bin/python", root),
        "/usr/bin/python"
    );
}

#[test]
fn test_resolve_process_command_keeps_commands_without_slash() {
    let root = Path::new("/tmp");
    assert_eq!(resolve_process_command("python", root), "python");
}

#[test]
fn test_resolve_process_command_resolves_relative_path() {
    let root = Path::new("/tmp/workflow");
    assert_eq!(
        resolve_process_command("bin/task", root),
        "/tmp/workflow/bin/task"
    );
}
