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
    /// Inline input type schema. Required when the task has dependencies.
    #[serde(default)]
    pub input_type: Option<serde_json::Value>,
    /// Inline output type schema. Required when any other task depends on this one.
    #[serde(default)]
    pub output_type: Option<serde_json::Value>,
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

        self.validate_types()?;

        Ok(())
    }

    /// Validate workflow type declarations:
    /// - Tasks that are depended upon must declare `output_type`
    /// - Tasks with `depends_on` must declare `input_type`
    /// - All type schemas must use valid type expressions
    fn validate_types(&self) -> Result<(), WorkflowValidationError> {
        // Build set of tasks that are depended upon, tracking who depends on them
        let mut depended_on: HashMap<&str, Vec<&str>> = HashMap::new();
        for (name, task) in &self.tasks {
            for dep in &task.depends_on {
                depended_on
                    .entry(dep.as_str())
                    .or_default()
                    .push(name.as_str());
            }
        }

        for (name, task) in &self.tasks {
            // If any task depends on this one, output_type is REQUIRED
            if let Some(dependents) = depended_on.get(name.as_str()) {
                if task.output_type.is_none() {
                    return Err(WorkflowValidationError::MissingOutputType {
                        task: name.clone(),
                        depended_by: dependents[0].to_string(),
                    });
                }
            }

            // If task has dependencies, input_type is REQUIRED
            if !task.depends_on.is_empty() && task.input_type.is_none() {
                return Err(WorkflowValidationError::MissingInputType {
                    task: name.clone(),
                });
            }

            // Validate type schema structure
            if let Some(ref schema) = task.output_type {
                validate_type_schema(schema, name, "output_type")?;
            }
            if let Some(ref schema) = task.input_type {
                validate_type_schema(schema, name, "input_type")?;
            }
        }

        Ok(())
    }
}

/// Valid primitive type names (language-agnostic).
const VALID_PRIMITIVES: &[&str] = &[
    "string", "integer", "number", "boolean", "date", "datetime",
];

/// Validate that a type schema value uses only valid type expressions:
/// - String leaf values must be a valid primitive type name
/// - Arrays must contain exactly one element (the element type)
/// - Objects recurse into their values
fn validate_type_schema(
    value: &serde_json::Value,
    task_name: &str,
    path: &str,
) -> Result<(), WorkflowValidationError> {
    match value {
        serde_json::Value::String(s) => {
            if !VALID_PRIMITIVES.contains(&s.as_str()) {
                return Err(WorkflowValidationError::InvalidTypeSchema {
                    task: task_name.to_string(),
                    path: path.to_string(),
                    reason: format!(
                        "unknown type '{}'. Valid types: {}",
                        s,
                        VALID_PRIMITIVES.join(", ")
                    ),
                });
            }
            Ok(())
        }
        serde_json::Value::Array(arr) => {
            if arr.len() != 1 {
                return Err(WorkflowValidationError::InvalidTypeSchema {
                    task: task_name.to_string(),
                    path: path.to_string(),
                    reason: format!(
                        "array type must have exactly one element (the element type), got {}",
                        arr.len()
                    ),
                });
            }
            validate_type_schema(&arr[0], task_name, &format!("{}[]", path))
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return Err(WorkflowValidationError::InvalidTypeSchema {
                    task: task_name.to_string(),
                    path: path.to_string(),
                    reason: "object type must have at least one field".to_string(),
                });
            }
            for (key, val) in map {
                validate_type_schema(val, task_name, &format!("{}.{}", path, key))?;
            }
            Ok(())
        }
        other => Err(WorkflowValidationError::InvalidTypeSchema {
            task: task_name.to_string(),
            path: path.to_string(),
            reason: format!(
                "expected a type name (string), array type, or object, got {:?}",
                other
            ),
        }),
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
            input_type: None,
            output_type: None,
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
                input_type: None,
                output_type: None,
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
                input_type: None,
                output_type: None,
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
                input_type: None,
                output_type: None,
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
                input_type: None,
                output_type: None,
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
        // Cycle detection happens before type validation
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

    // --- Type validation tests ---

    #[test]
    fn test_validate_types_requires_output_type_when_depended_upon() {
        let mut tasks = IndexMap::new();
        let a = process_task(); // no output_type
        let mut b = process_task();
        b.depends_on = vec!["a".to_string()];
        b.input_type = Some(serde_json::json!({"x": "integer"}));
        tasks.insert("a".to_string(), a);
        tasks.insert("b".to_string(), b);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(matches!(
            wf.validate(),
            Err(WorkflowValidationError::MissingOutputType { task, depended_by })
                if task == "a" && depended_by == "b"
        ));
    }

    #[test]
    fn test_validate_types_requires_input_type_when_has_depends_on() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"x": "integer"}));
        let mut b = process_task();
        b.depends_on = vec!["a".to_string()];
        // b has no input_type
        tasks.insert("a".to_string(), a);
        tasks.insert("b".to_string(), b);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(matches!(
            wf.validate(),
            Err(WorkflowValidationError::MissingInputType { task }) if task == "b"
        ));
    }

    #[test]
    fn test_validate_types_accepts_valid_typed_workflow() {
        let mut tasks = IndexMap::new();
        let mut extract = process_task();
        extract.output_type = Some(serde_json::json!({"users": ["string"], "count": "integer"}));
        let mut transform = process_task();
        transform.depends_on = vec!["extract".to_string()];
        transform.input_type =
            Some(serde_json::json!({"upstream": {"extract": {"users": ["string"], "count": "integer"}}}));
        tasks.insert("extract".to_string(), extract);
        tasks.insert("transform".to_string(), transform);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn test_validate_types_not_required_for_independent_tasks() {
        let mut tasks = IndexMap::new();
        tasks.insert("a".to_string(), process_task());
        tasks.insert("b".to_string(), process_task());

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn test_validate_types_leaf_task_output_type_optional() {
        // A leaf task (nothing depends on it) doesn't need output_type
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"x": "integer"}));
        let mut b = process_task();
        b.depends_on = vec!["a".to_string()];
        b.input_type = Some(serde_json::json!({"upstream": {"a": {"x": "integer"}}}));
        // b has no output_type — that's fine, nothing depends on b
        tasks.insert("a".to_string(), a);
        tasks.insert("b".to_string(), b);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn test_load_parses_typed_workflow_yaml() {
        let mut file = NamedTempFile::new().expect("tempfile should be created");
        let yaml = r#"
name: typed_workflow
tasks:
  extract:
    executor: process
    command: "echo extract"
    output_type:
      users: [string]
      count: integer
  transform:
    executor: process
    command: "echo transform"
    depends_on: [extract]
    input_type:
      upstream:
        extract:
          users: [string]
          count: integer
"#;
        std::io::Write::write_all(&mut file, yaml.as_bytes())
            .expect("yaml content should be written");

        let loaded = Workflow::load(file.path()).expect("typed workflow yaml should load");
        assert_eq!(loaded.name, "typed_workflow");
        assert!(loaded.tasks["extract"].output_type.is_some());
        assert!(loaded.tasks["transform"].input_type.is_some());
        let output_type = loaded.tasks["extract"].output_type.as_ref().unwrap();
        assert_eq!(output_type["count"], "integer");
    }

    // --- Type schema validation tests ---

    #[test]
    fn test_validate_type_schema_rejects_unknown_primitive() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"data": "str"})); // "str" is not valid, must be "string"
        tasks.insert("a".to_string(), a);

        // Need something to depend on "a" so output_type is checked
        let mut b = process_task();
        b.depends_on = vec!["a".to_string()];
        b.input_type = Some(serde_json::json!({"upstream": {"a": {"data": "string"}}}));
        tasks.insert("b".to_string(), b);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        let err = wf.validate().unwrap_err();
        assert!(matches!(err, WorkflowValidationError::InvalidTypeSchema { .. }));
        assert!(err.to_string().contains("str"));
    }

    #[test]
    fn test_validate_type_schema_accepts_all_primitives() {
        for prim in &["string", "integer", "number", "boolean", "date", "datetime"] {
            let mut tasks = IndexMap::new();
            let mut a = process_task();
            a.output_type = Some(serde_json::json!({"field": prim}));
            tasks.insert("a".to_string(), a);

            let wf = Workflow {
                name: "wf".to_string(),
                schedule: None,
                tasks,
            };
            assert!(wf.validate().is_ok(), "primitive '{}' should be valid", prim);
        }
    }

    #[test]
    fn test_validate_type_schema_accepts_array_types() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"users": ["string"], "scores": ["number"]}));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn test_validate_type_schema_rejects_empty_array() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"items": []}));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        let err = wf.validate().unwrap_err();
        assert!(matches!(err, WorkflowValidationError::InvalidTypeSchema { .. }));
        assert!(err.to_string().contains("exactly one element"));
    }

    #[test]
    fn test_validate_type_schema_rejects_multi_element_array() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"items": ["string", "integer"]}));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        let err = wf.validate().unwrap_err();
        assert!(matches!(err, WorkflowValidationError::InvalidTypeSchema { .. }));
    }

    #[test]
    fn test_validate_type_schema_accepts_nested_objects() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({
            "user": {
                "name": "string",
                "age": "integer",
                "tags": ["string"]
            }
        }));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn test_validate_type_schema_rejects_numeric_values() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({"count": 42}));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        let err = wf.validate().unwrap_err();
        assert!(matches!(err, WorkflowValidationError::InvalidTypeSchema { .. }));
    }

    #[test]
    fn test_validate_type_schema_accepts_date_and_datetime() {
        let mut tasks = IndexMap::new();
        let mut a = process_task();
        a.output_type = Some(serde_json::json!({
            "created": "date",
            "updated": "datetime",
            "timestamps": ["datetime"]
        }));
        tasks.insert("a".to_string(), a);

        let wf = Workflow {
            name: "wf".to_string(),
            schedule: None,
            tasks,
        };
        assert!(wf.validate().is_ok());
    }
}
