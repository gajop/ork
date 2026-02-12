use clap::Subcommand;

pub mod create_workflow;
pub mod create_workflow_yaml;
pub mod delete_workflow;
pub mod execute;
pub mod init;
pub mod list_workflows;
pub mod run;
pub mod run_workflow;
pub mod status;
pub mod tasks;
pub mod trigger;
pub mod validate_workflow;

pub use create_workflow::CreateWorkflow;
pub use create_workflow_yaml::CreateWorkflowYaml;
pub use delete_workflow::DeleteWorkflow;
pub use execute::Execute;
pub use init::Init;
pub use list_workflows::ListWorkflows;
pub use run::Run;
pub use run_workflow::RunWorkflow;
pub use status::Status;
pub use tasks::Tasks;
pub use trigger::Trigger;
pub use validate_workflow::ValidateWorkflow;

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database with migrations
    Init(Init),

    /// Start the scheduler (`ork run`) or execute a workflow file (`ork run <workflow.yaml>`)
    Run(Run),

    /// Create a new workflow
    CreateWorkflow(CreateWorkflow),

    /// Create a workflow from a YAML definition (DAG)
    CreateWorkflowYaml(CreateWorkflowYaml),

    /// List all workflows
    ListWorkflows(ListWorkflows),

    /// Delete a workflow (and all associated runs/tasks)
    DeleteWorkflow(DeleteWorkflow),

    /// Trigger a workflow run
    Trigger(Trigger),

    /// Show status of runs
    Status(Status),

    /// Show tasks for a run
    Tasks(Tasks),

    /// Execute a workflow from YAML file locally (create + trigger + run scheduler until complete)
    Execute(Execute),

    /// Create + trigger a workflow from a YAML file via the HTTP API
    RunWorkflow(RunWorkflow),

    /// Validate a workflow YAML file against the strict workflow schema
    ValidateWorkflow(ValidateWorkflow),
}

impl Commands {
    pub fn uses_api(&self) -> bool {
        matches!(
            self,
            Commands::RunWorkflow(_) | Commands::ValidateWorkflow(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uses_api_for_pre_db_commands() {
        let run_workflow = Commands::RunWorkflow(RunWorkflow {
            file: "wf.yaml".to_string(),
            api_url: "http://127.0.0.1:4000".to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        });
        assert!(run_workflow.uses_api());

        let validate = Commands::ValidateWorkflow(ValidateWorkflow {
            file: "wf.yaml".to_string(),
        });
        assert!(validate.uses_api());

        let non_api = Commands::Init(Init);
        assert!(!non_api.uses_api());
    }
}
