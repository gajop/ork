use ork_core::compiled::CompiledWorkflow;
use ork_core::database::NewWorkflowTask;
use ork_core::workflow::ExecutorKind;
use serde_json;
use std::path::Path;

pub fn build_workflow_tasks(compiled: &CompiledWorkflow) -> Vec<NewWorkflowTask> {
    let mut tasks = Vec::with_capacity(compiled.tasks.len());
    for (idx, task) in compiled.tasks.iter().enumerate() {
        let depends_on: Vec<String> = task
            .depends_on
            .iter()
            .filter_map(|dep_idx| compiled.tasks.get(*dep_idx).map(|t| t.name.clone()))
            .collect();

        let executor_type = match task.executor {
            ExecutorKind::CloudRun => "cloudrun",
            ExecutorKind::Process => "process",
            ExecutorKind::Python => "python",
            ExecutorKind::Library => "library",
        };

        let mut params = serde_json::Map::new();
        if !task.input.is_null() {
            params.insert("task_input".to_string(), task.input.clone());
        }
        params.insert(
            "max_retries".to_string(),
            serde_json::Value::Number(task.retries.into()),
        );
        params.insert(
            "timeout_seconds".to_string(),
            serde_json::Value::Number(task.timeout.into()),
        );

        match task.executor {
            ExecutorKind::CloudRun => {
                if let Some(job) = task.job.as_deref() {
                    params.insert(
                        "job_name".to_string(),
                        serde_json::Value::String(job.to_string()),
                    );
                }
            }
            ExecutorKind::Process => {
                if let Some(command) = task.command.as_deref() {
                    params.insert(
                        "command".to_string(),
                        serde_json::Value::String(resolve_process_command(command, &compiled.root)),
                    );
                } else if let Some(file) = task.file.as_ref() {
                    params.insert(
                        "command".to_string(),
                        serde_json::Value::String(file.to_string_lossy().to_string()),
                    );
                }
            }
            ExecutorKind::Python => {
                if let Some(file) = task.file.as_ref() {
                    params.insert(
                        "task_file".to_string(),
                        serde_json::Value::String(file.to_string_lossy().to_string()),
                    );
                }
                if let Some(module) = task.module.as_deref() {
                    params.insert(
                        "task_module".to_string(),
                        serde_json::Value::String(module.to_string()),
                    );
                }
                if let Some(function) = task.function.as_deref() {
                    params.insert(
                        "task_function".to_string(),
                        serde_json::Value::String(function.to_string()),
                    );
                }
                params.insert(
                    "python_path".to_string(),
                    serde_json::Value::String(compiled.root.to_string_lossy().to_string()),
                );
            }
            ExecutorKind::Library => {
                if let Some(file) = task.file.as_ref() {
                    params.insert(
                        "library_path".to_string(),
                        serde_json::Value::String(file.to_string_lossy().to_string()),
                    );
                }
            }
        }

        tasks.push(NewWorkflowTask {
            task_index: idx as i32,
            task_name: task.name.clone(),
            executor_type: executor_type.to_string(),
            depends_on,
            params: serde_json::Value::Object(params),
            signature: task.signature.clone(),
        });
    }

    tasks
}

fn resolve_process_command(command: &str, root: &Path) -> String {
    if command.chars().any(char::is_whitespace) {
        return command.to_string();
    }

    let path = Path::new(command);
    if !path.is_relative() || !command.contains('/') {
        return command.to_string();
    }

    root.join(path).to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::compiled::{CompiledTask, CompiledWorkflow};
    use std::path::PathBuf;

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
}
