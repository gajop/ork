use anyhow::Result;
use clap::Args;
use std::sync::Arc;

use ork_core::compiled::build_workflow_tasks;
use ork_core::database::Database;
use ork_core::workflow::Workflow as YamlWorkflow;

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

#[cfg(test)]
mod tests {
    use super::*;
    use ork_core::database::WorkflowRepository;
    use ork_state::SqliteDatabase;

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
    async fn test_create_workflow_from_yaml_uses_cwd_when_file_path_has_no_parent() {
        let db = SqliteDatabase::new(":memory:").await.expect("create db");
        db.run_migrations().await.expect("migrate");

        let yaml = r#"
name: wf-yaml-no-parent
tasks:
  first:
    executor: process
    command: "echo first"
"#;

        let workflow = create_workflow_from_yaml_str(&db, yaml, "/", "local", "local")
            .await
            .expect("create workflow from yaml");
        assert_eq!(workflow.name, "wf-yaml-no-parent");
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
}
