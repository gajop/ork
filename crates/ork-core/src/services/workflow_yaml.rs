use std::path::Path;

use thiserror::Error;

use crate::compiled::build_workflow_tasks;
use crate::database::Database;
use crate::models::Workflow as StoredWorkflow;
use crate::workflow::Workflow as YamlWorkflow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingWorkflowBehavior {
    Error,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyWorkflowYamlOutcome {
    Created,
    Replaced,
}

#[derive(Debug, Clone)]
pub struct ApplyWorkflowYamlResult {
    pub workflow: StoredWorkflow,
    pub outcome: ApplyWorkflowYamlOutcome,
}

pub struct ApplyWorkflowYamlRequest<'a> {
    pub yaml_content: &'a str,
    pub root: &'a Path,
    pub project: &'a str,
    pub region: &'a str,
    pub existing_workflow: ExistingWorkflowBehavior,
    pub persist_schedule_on_create: bool,
}

#[derive(Debug, Error)]
pub enum WorkflowServiceError {
    #[error("workflow already exists: {name}")]
    WorkflowAlreadyExists { name: String },

    #[error("invalid workflow yaml: {0}")]
    InvalidYaml(#[from] serde_yaml::Error),

    #[error("{0}")]
    InvalidWorkflow(String),

    #[error("{0}")]
    Compilation(String),

    #[error(transparent)]
    Storage(#[from] anyhow::Error),
}

pub async fn apply_workflow_yaml(
    db: &dyn Database,
    request: ApplyWorkflowYamlRequest<'_>,
) -> Result<ApplyWorkflowYamlResult, WorkflowServiceError> {
    let definition: YamlWorkflow = serde_yaml::from_str(request.yaml_content)?;
    definition
        .validate()
        .map_err(|err| WorkflowServiceError::InvalidWorkflow(err.to_string()))?;

    let compiled = definition
        .compile(request.root)
        .map_err(|err| WorkflowServiceError::Compilation(err.to_string()))?;

    let existing = match db.get_workflow(&definition.name).await {
        Ok(workflow) => Some(workflow),
        Err(err) if is_not_found_error(&err) => None,
        Err(err) => return Err(WorkflowServiceError::Storage(err)),
    };

    let (workflow, outcome) = match existing {
        Some(workflow) => match request.existing_workflow {
            ExistingWorkflowBehavior::Replace => (workflow, ApplyWorkflowYamlOutcome::Replaced),
            ExistingWorkflowBehavior::Error => {
                return Err(WorkflowServiceError::WorkflowAlreadyExists {
                    name: definition.name.clone(),
                });
            }
        },
        None => {
            let schedule = if request.persist_schedule_on_create {
                definition.schedule.as_deref()
            } else {
                None
            };
            let workflow = db
                .create_workflow(
                    &definition.name,
                    None,
                    "dag",
                    request.region,
                    request.project,
                    "dag",
                    None,
                    schedule,
                )
                .await?;
            (workflow, ApplyWorkflowYamlOutcome::Created)
        }
    };

    let workflow_tasks = build_workflow_tasks(&compiled);
    db.create_workflow_tasks(workflow.id, &workflow_tasks)
        .await?;

    Ok(ApplyWorkflowYamlResult { workflow, outcome })
}

fn is_not_found_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("row not found") || msg.contains("no rows") || msg.contains("not found")
}
