use super::*;
use crate::workflow::Workflow;
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
