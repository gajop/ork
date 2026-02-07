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
    #[serde(default)]
    pub schedule: Option<String>,
    pub tasks: IndexMap<String, TaskDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskDefinition {
    pub executor: ExecutorKind,
    pub file: Option<PathBuf>,
    pub command: Option<String>,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub retries: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutorKind {
    #[serde(alias = "shell")]
    Process,
    #[serde(alias = "cloud_run")]
    CloudRun,
    Python,
    Library,
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

            match task.executor {
                ExecutorKind::Python => {
                    let has_module = task
                        .module
                        .as_deref()
                        .map(|module| !module.is_empty())
                        .unwrap_or(false);
                    if task.file.is_none() && !has_module {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::Process => {
                    if task.command.is_none() && task.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskCommand {
                            task: name.clone(),
                        });
                    }
                }
                ExecutorKind::CloudRun => {
                    if task.job.as_deref().unwrap_or_default().is_empty() {
                        return Err(WorkflowValidationError::MissingTaskJob { task: name.clone() });
                    }
                }
                ExecutorKind::Library => {
                    if task.file.is_none() {
                        return Err(WorkflowValidationError::MissingTaskFile {
                            task: name.clone(),
                        });
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::WorkflowValidationError;
    use tempfile::NamedTempFile;

    fn process_task() -> TaskDefinition {
        TaskDefinition {
            executor: ExecutorKind::Process,
            file: None,
            command: Some("echo ok".to_string()),
            job: None,
            module: None,
            function: None,
            input: serde_json::json!({}),
            depends_on: Vec::new(),
            timeout: 300,
            retries: 0,
        }
    }

    #[test]
    fn test_validate_rejects_empty_workflow_name_and_no_tasks() {
        let empty_name = Workflow {
            name: "  ".to_string(),
            schedule: None,
            tasks: IndexMap::new(),
        };
        assert_eq!(
            empty_name.validate(),
            Err(WorkflowValidationError::EmptyWorkflowName)
        );

        let no_tasks = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks: IndexMap::new(),
        };
        assert_eq!(no_tasks.validate(), Err(WorkflowValidationError::NoTasks));
    }

    #[test]
    fn test_validate_rejects_dependency_errors() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.depends_on = vec!["missing".to_string()];
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(matches!(
            wf.validate(),
            Err(WorkflowValidationError::UnknownDependency { .. })
        ));

        let mut tasks = IndexMap::new();
        let mut self_dep = process_task();
        self_dep.depends_on = vec!["a".to_string()];
        tasks.insert("a".to_string(), self_dep);
        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(matches!(
            wf.validate(),
            Err(WorkflowValidationError::SelfDependency { task }) if task == "a"
        ));
    }

    #[test]
    fn test_validate_rejects_executor_specific_missing_fields() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "python".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Python,
                file: None,
                command: None,
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 0,
            },
        );
        assert!(matches!(
            (Workflow {
                name: "wf".to_string(),
                schedule: None,
                tasks,
            })
            .validate(),
            Err(WorkflowValidationError::MissingTaskFile { .. })
        ));

        let mut tasks = IndexMap::new();
        tasks.insert(
            "process".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Process,
                file: None,
                command: None,
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 0,
            },
        );
        assert!(matches!(
            (Workflow {
                name: "wf".to_string(),
                schedule: None,
                tasks,
            })
            .validate(),
            Err(WorkflowValidationError::MissingTaskCommand { .. })
        ));

        let mut tasks = IndexMap::new();
        tasks.insert(
            "cloudrun".to_string(),
            TaskDefinition {
                executor: ExecutorKind::CloudRun,
                file: None,
                command: None,
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 0,
            },
        );
        assert!(matches!(
            (Workflow {
                name: "wf".to_string(),
                schedule: None,
                tasks,
            })
            .validate(),
            Err(WorkflowValidationError::MissingTaskJob { .. })
        ));

        let mut tasks = IndexMap::new();
        tasks.insert(
            "lib".to_string(),
            TaskDefinition {
                executor: ExecutorKind::Library,
                file: None,
                command: None,
                job: None,
                module: None,
                function: None,
                input: serde_json::json!({}),
                depends_on: Vec::new(),
                timeout: 300,
                retries: 0,
            },
        );
        assert!(matches!(
            (Workflow {
                name: "wf".to_string(),
                schedule: None,
                tasks,
            })
            .validate(),
            Err(WorkflowValidationError::MissingTaskFile { .. })
        ));
    }

    #[test]
    fn test_validate_detects_cycle() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        let mut b = process_task();
        a.depends_on = vec!["b".to_string()];
        b.depends_on = vec!["a".to_string()];
        tasks.insert("a".to_string(), a);
        tasks.insert("b".to_string(), b);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(matches!(
            wf.validate(),
            Err(WorkflowValidationError::Cycle { .. })
        ));
    }

    #[test]
    fn test_load_parses_valid_yaml() {
        let mut file = NamedTempFile::new().expect("tempfile should be created");
        let yaml = r#"
name: hello
tasks:
  first:
    executor: process
    command: "echo hello"
"#;
        std::io::Write::write_all(&mut file, yaml.as_bytes())
            .expect("yaml content should be written");

        let loaded = Workflow::load(file.path()).expect("workflow yaml should load");
        assert_eq!(loaded.name, "hello");
        assert!(loaded.tasks.contains_key("first"));
    }
}
