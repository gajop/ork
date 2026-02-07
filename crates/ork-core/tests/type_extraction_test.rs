//! Tests for Python task type extraction during workflow compilation.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use ork_core::workflow::Workflow;

/// Creates a temporary Python file with the given content and returns its path.
fn create_temp_python_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let path = dir.path().join(filename);
    fs::write(&path, content).expect("Failed to write temp file");
    path
}

/// Creates a workflow YAML string for testing.
fn create_workflow_yaml(task_file: &str, function: &str) -> String {
    format!(
        r#"
name: test_workflow
tasks:
  test_task:
    executor: python
    file: {task_file}
    function: {function}
    input:
      x: 1
"#,
        task_file = task_file,
        function = function
    )
}

#[test]
fn test_typed_python_task_compiles_with_signature() {
    let temp_dir = TempDir::new().unwrap();

    // Create a properly typed Python function
    let py_content = r#"
def add(a: int, b: int) -> int:
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "typed_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should succeed since all arguments are typed
    let result = workflow.compile(temp_dir.path());

    assert!(result.is_ok(), "Compilation should succeed for typed tasks");

    let compiled = result.unwrap();
    assert_eq!(compiled.tasks.len(), 1);

    let task = &compiled.tasks[0];
    assert!(task.signature.is_some(), "Signature should be extracted");

    let sig = task.signature.as_ref().unwrap();
    assert!(sig.get("inputs").is_some(), "Signature should have inputs");
    assert!(sig.get("output").is_some(), "Signature should have output");

    let inputs = sig.get("inputs").unwrap().as_object().unwrap();
    assert_eq!(inputs.get("a").unwrap().as_str().unwrap(), "int");
    assert_eq!(inputs.get("b").unwrap().as_str().unwrap(), "int");
    assert_eq!(sig.get("output").unwrap().as_str().unwrap(), "int");
}

#[test]
fn test_untyped_python_task_fails_compilation() {
    let temp_dir = TempDir::new().unwrap();

    // Create a Python function with untyped arguments
    let py_content = r#"
def untyped_add(a, b):
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "untyped_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "untyped_add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should fail because arguments are not typed
    let result = workflow.compile(temp_dir.path());

    assert!(result.is_err(), "Compilation should fail for untyped tasks");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("untyped") || err_msg.contains("Introspection"),
        "Error message should mention untyped arguments or introspection failure: {}",
        err_msg
    );
}

#[test]
fn test_partially_typed_python_task_fails_compilation() {
    let temp_dir = TempDir::new().unwrap();

    // Create a Python function with only some typed arguments
    let py_content = r#"
def partial_add(a: int, b) -> int:
    return a + b
"#;
    let py_path = create_temp_python_file(&temp_dir, "partial_task.py", py_content);

    let yaml = create_workflow_yaml(py_path.to_str().unwrap(), "partial_add");
    let workflow: Workflow = serde_yaml::from_str(&yaml).unwrap();

    // This should fail because 'b' is not typed
    let result = workflow.compile(temp_dir.path());

    assert!(
        result.is_err(),
        "Compilation should fail for partially typed tasks"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("'b'") || err_msg.contains("untyped"),
        "Error message should mention the untyped argument 'b': {}",
        err_msg
    );
}
