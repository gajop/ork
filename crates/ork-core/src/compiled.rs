use std::path::{Path, PathBuf};

use indexmap::IndexMap;

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
    pub depends_on: Vec<usize>,
    pub timeout: u64,
    pub retries: u32,
    pub signature: Option<serde_json::Value>,
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
                    if let Some(file) = task.file.as_ref() {
                        Some(resolve_file(root, name, file)?)
                    } else {
                        None
                    }
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
                depends_on,
                timeout: task.timeout,
                retries: task.retries,
                signature,
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
            if let Some(entry) = indegree.get_mut(child) {
                *entry -= 1;
                if *entry == 0 {
                    queue.push_back(child);
                }
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
    if let Ok(value) = parsed {
        if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
            return Err(format!("Task analysis error: {}", err));
        }
        if output.status.success() {
            return Ok(value);
        }
    } else if output.status.success() {
        let parse_err = parsed.expect_err("parse should fail in this branch");
        return Err(format!("Failed to parse introspection output: {}", parse_err));
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Introspection failed ({}): {}",
            func_name,
            stderr.trim()
        ));
    }
    Err("Unexpected introspection state".to_string())
}

#[cfg(test)]
mod tests;
