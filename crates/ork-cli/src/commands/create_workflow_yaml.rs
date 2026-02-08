use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::database::{Database, NewWorkflowTask};
use ork_core::workflow::{ExecutorKind, Workflow as YamlWorkflow};

#[derive(Args)]
pub struct CreateWorkflowYaml {
    /// Path to workflow YAML file
    #[arg(short, long)]
    pub file: String,

    /// Project label for the workflow (default: local)
    #[arg(short, long, default_value = "local")]
    pub project: String,

    /// Region label for the workflow (default: local)
    #[arg(short, long, default_value = "local")]
    pub region: String,
}

impl CreateWorkflowYaml {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        let yaml_content = std::fs::read_to_string(&self.file)?;
        let workflow = create_workflow_from_yaml_str(
            &*db,
            &yaml_content,
            &self.file,
            &self.project,
            &self.region,
        )
        .await?;

        println!("✓ Created workflow from YAML: {}", workflow.name);
        println!("  ID: {}", workflow.id);
        println!("  Executor: {}", workflow.executor_type);
        println!("  Project: {}", workflow.project);
        println!("  Region: {}", workflow.region);
        Ok(())
    }
}

pub async fn create_workflow_from_yaml_str(
    db: &dyn Database,
    yaml_content: &str,
    file_path: &str,
    project: &str,
    region: &str,
) -> Result<ork_core::models::Workflow> {
    let definition: YamlWorkflow = serde_yaml::from_str(yaml_content)?;
    definition
        .validate()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let root = std::path::Path::new(file_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
    let root = root.canonicalize().unwrap_or(root);
    let compiled = definition
        .compile(&root)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let workflow = db
        .create_workflow(
            &definition.name,
            None,
            "dag",
            region,
            project,
            "dag",
            None,
            None,
        )
        .await?;

    let workflow_tasks = build_workflow_tasks(&compiled);
    db.create_workflow_tasks(workflow.id, &workflow_tasks)
        .await?;

    Ok(workflow)
}

fn build_workflow_tasks(compiled: &ork_core::compiled::CompiledWorkflow) -> Vec<NewWorkflowTask> {
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
                        serde_json::Value::String(command.to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::compiled::{CompiledTask, CompiledWorkflow};
    use ork_core::database::Database;
    use ork_core::workflow::{TaskDefinition, Workflow};
    use ork_state::SqliteDatabase;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_create_workflow_from_yaml_str_creates_workflow_and_tasks() {
        let db = SqliteDatabase::new(":memory:").await.expect("create db");
        db.run_migrations().await.expect("migrate");

        let yaml = r#"
name: wf-yaml
tasks:
  first:
    executor: process
    command: "echo first"
  second:
    executor: process
    command: "echo second"
    depends_on: ["first"]
"#;

        let workflow =
            create_workflow_from_yaml_str(&db, yaml, "/tmp/workflow.yaml", "local", "local")
                .await
                .expect("create workflow from yaml");
        assert_eq!(workflow.name, "wf-yaml");

        let tasks = db
            .list_workflow_tasks(workflow.id)
            .await
            .expect("list workflow tasks");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].depends_on, vec!["first".to_string()]);
    }

    #[tokio::test]
    async fn test_create_workflow_yaml_command_execute() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let path = std::env::temp_dir().join(format!(
            "ork-create-workflow-yaml-{}.yaml",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &path,
            r#"
name: wf-yaml-cmd
tasks:
  first:
    executor: process
    command: "echo first"
"#,
        )
        .expect("write temp workflow");

        CreateWorkflowYaml {
            file: path.to_string_lossy().to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
        }
        .execute(db.clone())
        .await
        .expect("command execute should succeed");

        let workflow = db
            .get_workflow("wf-yaml-cmd")
            .await
            .expect("workflow should exist");
        let tasks = db
            .list_workflow_tasks(workflow.id)
            .await
            .expect("list tasks");
        assert_eq!(tasks.len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_build_workflow_tasks_maps_process_command_and_dependencies() {
        let mut workflow_tasks = indexmap::IndexMap::new();
        workflow_tasks.insert(
            "a".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo a".to_string()),
                job: None,
                module: None,
                function: None,
                input: serde_json::Value::Null,
                depends_on: vec![],
                timeout: 5,
                retries: 1,
            },
        );
        workflow_tasks.insert(
            "b".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Process,
                file: None,
                command: Some("echo b".to_string()),
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({"x": 1}),
                depends_on: vec!["a".to_string()],
                timeout: 7,
                retries: 2,
            },
        );
        let workflow = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks: workflow_tasks,
        };

        let compiled = workflow
            .compile(std::path::Path::new("."))
            .expect("compile workflow");
        let tasks = super::build_workflow_tasks(&compiled);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].executor_type, "process");
        assert_eq!(tasks[1].depends_on, vec!["a".to_string()]);
        assert_eq!(tasks[1].params["task_input"], serde_json::json!({"x": 1}));
    }

    #[test]
    fn test_build_workflow_tasks_maps_all_executor_specific_params() {
        let compiled = CompiledWorkflow {
            name: "wf".to_string(),
            tasks: vec![
                CompiledTask {
                    name: "cloud".to_string(),
                    executor: ExecutorKind::CloudRun,
                    file: None,
                    command: None,
                    job: Some("job-a".to_string()),
                    module: None,
                    function: None,
                    input: serde_json::Value::Null,
                    depends_on: vec![],
                    timeout: 10,
                    retries: 0,
                    signature: None,
                },
                CompiledTask {
                    name: "python".to_string(),
                    executor: ExecutorKind::Python,
                    file: None,
                    command: None,
                    job: None,
                    module: Some("pkg.tasks".to_string()),
                    function: Some("run".to_string()),
                    input: serde_json::json!({"v": 1}),
                    depends_on: vec![0],
                    timeout: 20,
                    retries: 1,
                    signature: None,
                },
                CompiledTask {
                    name: "lib".to_string(),
                    executor: ExecutorKind::Library,
                    file: Some(PathBuf::from("/tmp/libtask.so")),
                    command: None,
                    job: None,
                    module: None,
                    function: None,
                    input: serde_json::Value::Null,
                    depends_on: vec![1],
                    timeout: 30,
                    retries: 2,
                    signature: None,
                },
            ],
            name_index: indexmap::IndexMap::new(),
            topo: vec![0, 1, 2],
            root: PathBuf::from("/tmp"),
        };

        let tasks = super::build_workflow_tasks(&compiled);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].params["job_name"], serde_json::json!("job-a"));
        assert_eq!(
            tasks[1].params["task_module"],
            serde_json::json!("pkg.tasks")
        );
        assert_eq!(tasks[1].params["task_function"], serde_json::json!("run"));
        assert_eq!(tasks[1].params["python_path"], serde_json::json!("/tmp"));
        assert_eq!(
            tasks[2].params["library_path"],
            serde_json::json!("/tmp/libtask.so")
        );
    }
}
