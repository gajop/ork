use clap::Subcommand;

pub mod create_workflow;
pub mod create_workflow_yaml;
pub mod delete_workflow;
pub mod init;
pub mod list_workflows;
pub mod run;
pub mod run_workflow;
pub mod status;
pub mod tasks;
pub mod trigger;

pub use create_workflow::CreateWorkflow;
pub use create_workflow_yaml::CreateWorkflowYaml;
pub use delete_workflow::DeleteWorkflow;
pub use init::Init;
pub use list_workflows::ListWorkflows;
pub use run::Run;
pub use run_workflow::RunWorkflow;
pub use status::Status;
pub use tasks::Tasks;
pub use trigger::Trigger;

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize the database with migrations
    Init(Init),

    /// Start the orchestrator scheduler
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

    /// Create + trigger a workflow from a YAML file via the HTTP API
    RunWorkflow(RunWorkflow),
}

impl Commands {
    pub fn uses_api(&self) -> bool {
        matches!(self, Commands::RunWorkflow(_))
    }
}
