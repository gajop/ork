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
        let yaml = std::fs::read_to_string(&self.file)?;
        let definition: YamlWorkflow = serde_yaml::from_str(&yaml)?;
        definition
            .validate()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        let root = std::path::Path::new(&self.file)
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
                &self.region,
                &self.project,
                "dag",
                None,
            )
            .await?;

        let workflow_tasks = build_workflow_tasks(&compiled);
        db.create_workflow_tasks(workflow.id, &workflow_tasks)
            .await?;

        println!("✓ Created workflow from YAML: {}", workflow.name);
        println!("  ID: {}", workflow.id);
        println!("  Executor: {}", workflow.executor_type);
        println!("  Project: {}", workflow.project);
        println!("  Region: {}", workflow.region);
        Ok(())
    }
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
            ExecutorKind::Process | ExecutorKind::Python => "process",
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
        if !task.env.is_empty() {
            let env_json = task
                .env
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect::<serde_json::Map<_, _>>();
            params.insert("env".to_string(), serde_json::Value::Object(env_json));
        }

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
        }

        tasks.push(NewWorkflowTask {
            task_index: idx as i32,
            task_name: task.name.clone(),
            executor_type: executor_type.to_string(),
            depends_on,
            params: serde_json::Value::Object(params),
        });
    }

    tasks
}
