use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::database::NewWorkflowTask;
use crate::error::{OrkError, OrkResult, WorkflowValidationError};
use crate::workflow::{ExecutorKind, Workflow};

#[derive(Debug, Clone)]
pub struct CompiledWorkflow {
    pub name: String,
    pub tasks: Vec<CompiledTask>,
    pub name_index: IndexMap<String, usize>,
    pub topo: Vec<usize>,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompiledTask {
    pub name: String,
    pub executor: ExecutorKind,
    pub file: Option<PathBuf>,
    pub command: Option<String>,
    pub job: Option<String>,
    pub module: Option<String>,
    pub function: Option<String>,
    pub input: serde_json::Value,
    pub inputs: serde_json::Value,
    pub depends_on: Vec<usize>,
    pub timeout: u64,
    pub retries: u32,
    pub signature: Option<serde_json::Value>,
    pub input_type: Option<serde_json::Value>,
    pub output_type: Option<serde_json::Value>,
}

impl Workflow {
    pub fn compile(&self, root: &Path) -> OrkResult<CompiledWorkflow> {
        // validate once before compiling
        self.validate().map_err(OrkError::InvalidWorkflow)?;

        let mut name_index: IndexMap<String, usize> = IndexMap::new();
        for (idx, name) in self.tasks.keys().enumerate() {
            name_index.insert(name.clone(), idx);
        }

        let mut tasks = Vec::with_capacity(self.tasks.len());
        for (name, task) in &self.tasks {
            let mut signature = None;
            let file = match task.executor {
                ExecutorKind::Python => {
                    let task_file = if let Some(file) = task.file.as_ref() {
                        Some(resolve_file(root, name, file)?)
                    } else {
                        None
                    };

                    if let Some(path) = task_file.as_ref() {
                        let func_name = task.function.as_deref().unwrap_or("main");
                        signature =
                            Some(introspect_python_signature(path, func_name).map_err(|e| {
                                OrkError::InvalidWorkflow(WorkflowValidationError::Custom(e))
                            })?);
                    }
                    task_file
                }
                ExecutorKind::Process => {
                    if let Some(file) = task.file.as_ref() {
                        Some(resolve_file(root, name, file)?)
                    } else {
                        None
                    }
                }
                ExecutorKind::Library => {
                    let file = task.file.as_ref().expect("validated library task file");
                    Some(resolve_file(root, name, file)?)
                }
                ExecutorKind::CloudRun => None,
            };
            let depends_on: Vec<usize> = task
                .depends_on
                .iter()
                .map(|d| *name_index.get(d).expect("validated dependency"))
                .collect();
            tasks.push(CompiledTask {
                name: name.clone(),
                executor: task.executor.clone(),
                file,
                command: task.command.clone(),
                job: task.job.clone(),
                module: task.module.clone(),
                function: task.function.clone(),
                input: task.input.clone(),
                inputs: task.inputs.clone(),
                depends_on,
                timeout: task.timeout,
                retries: task.retries,
                signature,
                input_type: task.input_type.clone(),
                output_type: task.output_type.clone(),
            });
        }

        let topo = topo_sort(&tasks)?;

        Ok(CompiledWorkflow {
            name: self.name.clone(),
            tasks,
            name_index,
            topo,
            root: root.to_path_buf(),
        })
    }
}

fn resolve_file(root: &Path, task_name: &str, file: &Path) -> OrkResult<PathBuf> {
    let resolved = if file.is_relative() {
        root.join(file)
    } else {
        file.to_path_buf()
    };
    resolved.canonicalize().map_err(|_| {
        OrkError::InvalidWorkflow(WorkflowValidationError::TaskFileNotFound {
            task: task_name.to_string(),
            path: resolved,
        })
    })
}

fn topo_sort(tasks: &[CompiledTask]) -> OrkResult<Vec<usize>> {
    let mut indegree: Vec<usize> = vec![0; tasks.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); tasks.len()];

    for (idx, task) in tasks.iter().enumerate() {
        indegree[idx] = task.depends_on.len();
        for &dep in &task.depends_on {
            dependents[dep].push(idx);
        }
    }

    let mut queue: std::collections::VecDeque<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, &deg)| if deg == 0 { Some(idx) } else { None })
        .collect();

    let mut topo = Vec::with_capacity(tasks.len());
    while let Some(node) = queue.pop_front() {
        topo.push(node);
        for &child in &dependents[node] {
            let entry = &mut indegree[child];
            *entry -= 1;
            if *entry == 0 {
                queue.push_back(child);
            }
        }
    }

    if topo.len() != tasks.len() {
        return Err(OrkError::InvalidWorkflow(WorkflowValidationError::Cycle {
            task: "<unknown>".into(),
        }));
    }

    Ok(topo)
}

/// Embedded Python introspection script
const INSPECT_TASK_PY: &str = include_str!("../../ork-executors/runtime/python/inspect_task.py");

fn get_inspect_script() -> std::result::Result<PathBuf, String> {
    use std::fs;

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("ork")
        .join("runtime");

    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create runtime cache dir: {}", e))?;

    let script_path = cache_dir.join("inspect_task.py");

    if !script_path.exists() {
        fs::write(&script_path, INSPECT_TASK_PY)
            .map_err(|e| format!("Failed to write introspection script: {}", e))?;
    }

    Ok(script_path)
}

fn introspect_python_signature(
    path: &Path,
    func_name: &str,
) -> std::result::Result<serde_json::Value, String> {
    use std::process::Command;

    let script_path = get_inspect_script()?;

    let output = Command::new("python3")
        .arg(&script_path)
        .arg(path)
        .arg(func_name)
        .output()
        .map_err(|e| format!("Failed to execute introspection script: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = serde_json::from_str::<serde_json::Value>(&stdout);
    if let Ok(value) = parsed.as_ref()
        && let Some(err) = value.get("error").and_then(|v| v.as_str())
    {
        return Err(format!("Task analysis error: {}", err));
    }

    if output.status.success() {
        let value = if let Ok(value) = parsed {
            value
        } else {
            let parse_err = parsed.expect_err("parse should fail in this branch");
            return Err(format!(
                "Failed to parse introspection output: {}",
                parse_err
            ));
        };
        return Ok(value);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "Introspection failed ({}): {}",
        func_name,
        stderr.trim()
    ))
}

/// Build workflow tasks from a compiled workflow
///
/// This function converts a `CompiledWorkflow` into a vector of `NewWorkflowTask` rows
/// that can be inserted into the database. It handles:
/// - Dependency mapping from indices to task names
/// - Executor-specific parameter population (process, python, cloudrun, library)
/// - Relative process command resolution
/// - Python path propagation
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
        if !task.inputs.is_null() {
            params.insert("task_bindings".to_string(), task.inputs.clone());
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

        // Merge type info into the signature
        let mut signature = task.signature.clone().unwrap_or(serde_json::json!({}));
        if let Some(ref input_type) = task.input_type {
            signature["input_type"] = input_type.clone();
        }
        if let Some(ref output_type) = task.output_type {
            signature["output_type"] = output_type.clone();
        }
        let signature = if signature == serde_json::json!({}) {
            None
        } else {
            Some(signature)
        };

        tasks.push(NewWorkflowTask {
            task_index: idx as i32,
            task_name: task.name.clone(),
            executor_type: executor_type.to_string(),
            depends_on,
            params: serde_json::Value::Object(params),
            signature,
        });
    }

    tasks
}

/// Resolve relative process commands to absolute paths
///
/// If the command contains whitespace, it's assumed to be a shell command and returned as-is.
/// If the command is an absolute path or doesn't contain '/', it's returned as-is.
/// Otherwise, it's resolved relative to the workflow root.
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
mod tests;
