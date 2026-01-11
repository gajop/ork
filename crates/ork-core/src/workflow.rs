use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{OrkError, OrkResult, WorkflowValidationError};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Workflow {
    pub name: String,
    pub tasks: IndexMap<String, TaskDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDefinition {
    pub executor: ExecutorKind,
    pub file: Option<PathBuf>,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutorKind {
    Python,
}

fn default_timeout_secs() -> u64 {
    300
}

fn default_retries() -> u32 {
    0
}

impl Workflow {
    pub fn load(path: &Path) -> OrkResult<Self> {
        let data = std::fs::read_to_string(path).map_err(|source| OrkError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

        let workflow: Workflow =
            serde_yaml::from_str(&data).map_err(|source| OrkError::YamlParse {
                path: path.to_path_buf(),
                source,
            })?;

        workflow.validate().map_err(OrkError::InvalidWorkflow)?;
        Ok(workflow)
    }

    pub fn validate(&self) -> Result<(), WorkflowValidationError> {
        if self.name.trim().is_empty() {
            return Err(WorkflowValidationError::EmptyWorkflowName);
        }
        if self.tasks.is_empty() {
            return Err(WorkflowValidationError::NoTasks);
        }

        for (name, task) in &self.tasks {
            if name.trim().is_empty() {
                return Err(WorkflowValidationError::EmptyTaskName);
            }

            for dep in &task.depends_on {
                if !self.tasks.contains_key(dep) {
                    return Err(WorkflowValidationError::UnknownDependency {
                        task: name.clone(),
                        dependency: dep.clone(),
                    });
                }
                if dep == name {
                    return Err(WorkflowValidationError::SelfDependency { task: name.clone() });
                }
            }

            if task.file.is_none() {
                return Err(WorkflowValidationError::MissingTaskFile { task: name.clone() });
            }
        }

        if let Some(task) = detect_cycle(self) {
            return Err(WorkflowValidationError::Cycle { task });
        }

        Ok(())
    }
}

fn detect_cycle(workflow: &Workflow) -> Option<String> {
    fn visit(workflow: &Workflow, name: &str, visiting: &mut HashMap<String, bool>) -> bool {
        match visiting.get(name) {
            Some(true) => return true,
            Some(false) => return false,
            None => {}
        }

        visiting.insert(name.to_string(), true);
        if let Some(task) = workflow.tasks.get(name) {
            for dep in &task.depends_on {
                if visit(workflow, dep, visiting) {
                    return true;
                }
            }
        }
        visiting.insert(name.to_string(), false);
        false
    }

    let mut visiting = HashMap::new();
    for name in workflow.tasks.keys() {
        if visit(workflow, name, &mut visiting) {
            return Some(name.clone());
        }
    }
    None
}
