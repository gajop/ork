use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::error::{OrkError, OrkResult, WorkflowValidationError};
use crate::workflow::{ExecutorKind, TaskDefinition, Workflow};

#[derive(Debug, Clone)]
pub struct CompiledWorkflow {
    pub name: String,
    pub tasks: Vec<CompiledTask>,
    pub name_index: IndexMap<String, usize>,
    pub topo: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CompiledTask {
    pub name: String,
    pub executor: ExecutorKind,
    pub file: PathBuf,
    pub input: serde_json::Value,
    pub depends_on: Vec<usize>,
    pub timeout: u64,
    pub retries: u32,
    pub env: std::collections::HashMap<String, String>,
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
            let file = resolve_file(root, name, task)?;
            let depends_on: Vec<usize> = task
                .depends_on
                .iter()
                .map(|d| *name_index.get(d).expect("validated dependency"))
                .collect();
            tasks.push(CompiledTask {
                name: name.clone(),
                executor: task.executor.clone(),
                file,
                input: task.input.clone(),
                depends_on,
                timeout: task.timeout,
                retries: task.retries,
                env: task.env.clone(),
            });
        }

        let topo = topo_sort(&tasks)?;

        Ok(CompiledWorkflow {
            name: self.name.clone(),
            tasks,
            name_index,
            topo,
        })
    }
}

fn resolve_file(root: &Path, task_name: &str, task: &TaskDefinition) -> OrkResult<PathBuf> {
    let file = task.file.as_ref().ok_or_else(|| {
        OrkError::InvalidWorkflow(WorkflowValidationError::MissingTaskFile {
            task: task_name.to_string(),
        })
    })?;
    let resolved = if file.is_relative() {
        root.join(file)
    } else {
        file.clone()
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
