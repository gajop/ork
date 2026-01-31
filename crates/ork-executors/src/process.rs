use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};
use uuid::Uuid;

use ork_core::executor::{Executor, StatusUpdate};

#[derive(Debug, Clone)]
enum ProcessStatus {
    Running,
    Success,
    Failed,
}

pub struct ProcessExecutor {
    working_dir: String,
    process_states: Arc<RwLock<HashMap<String, ProcessStatus>>>,
    status_tx: Arc<RwLock<Option<mpsc::UnboundedSender<StatusUpdate>>>>,
}

impl ProcessExecutor {
    pub fn new(working_dir: Option<String>) -> Self {
        Self {
            working_dir: working_dir.unwrap_or_else(|| ".".to_string()),
            process_states: Arc::new(RwLock::new(HashMap::new())),
            status_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn execute_process(
        &self,
        task_id: Uuid,
        command: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        let execution_id = Uuid::new_v4().to_string();
        let command = command.to_string();

        info!(
            "Executing process: {} with execution_id: {}",
            command, execution_id
        );

        let (
            mut env_vars,
            task_file,
            task_module,
            task_function,
            task_input,
            runner_path,
            python_path,
        ) = build_env_vars(params);

        // Add execution ID to env
        env_vars.insert("EXECUTION_ID".to_string(), execution_id.clone());

        // Mark as running
        {
            let mut states = self.process_states.write().await;
            states.insert(execution_id.clone(), ProcessStatus::Running);
        }

        // Clone for the async task
        let exec_id_clone = execution_id.clone();
        let states_clone = self.process_states.clone();
        let status_tx_clone = self.status_tx.clone();
        let working_dir = self.working_dir.clone();
        let command = command.clone();

        tokio::spawn(async move {
            let mut cmd = if let Some(task_file) = task_file.as_ref() {
                let mut python_cmd = build_python_command(task_file, python_path.as_ref());
                if let Some(runner_path) = runner_path.or_else(find_python_runner) {
                    python_cmd.arg(runner_path);
                } else {
                    warn!(
                        "Python runner not found. Set runner_path in params or run from repo root."
                    );
                    python_cmd.arg("scripts/run_python_task.py");
                }
                python_cmd
            } else if task_module.is_some() {
                let mut python_cmd = build_python_command(Path::new("."), python_path.as_ref());
                if let Some(runner_path) = runner_path.or_else(find_python_runner) {
                    python_cmd.arg(runner_path);
                } else {
                    warn!(
                        "Python runner not found. Set runner_path in params or run from repo root."
                    );
                    python_cmd.arg("scripts/run_python_task.py");
                }
                python_cmd
            } else {
                let command_path = resolve_command_path(&working_dir, &command);
                let mut shell_cmd = Command::new("sh");
                shell_cmd.arg("-c").arg(&command_path);
                shell_cmd
            };

            if let Some(task_file) = task_file.as_ref() {
                env_vars.insert(
                    "ORK_TASK_FILE".to_string(),
                    task_file.to_string_lossy().to_string(),
                );
                if let Some(input) = task_input {
                    env_vars.insert("ORK_INPUT_JSON".to_string(), input.to_string());
                }
                if let Some(name) = env_vars.get("task_name").cloned() {
                    env_vars.insert("ORK_TASK_NAME".to_string(), name);
                }
                if let Some(name) = env_vars.get("workflow_name").cloned() {
                    env_vars.insert("ORK_WORKFLOW_NAME".to_string(), name);
                }
                if let Some(run_id) = env_vars.get("run_id").cloned() {
                    env_vars.insert("ORK_RUN_ID".to_string(), run_id);
                }
                if let Some(root) = python_path.as_ref().cloned().or_else(|| find_project_root(task_file)) {
                    cmd.current_dir(&root);
                    env_vars.insert("PYTHONPATH".to_string(), root.to_string_lossy().to_string());
                }
            } else if let Some(module) = task_module.as_ref() {
                env_vars.insert("ORK_TASK_MODULE".to_string(), module.to_string());
                if let Some(function) = task_function.as_ref() {
                    env_vars.insert("ORK_TASK_FUNCTION".to_string(), function.to_string());
                }
                if let Some(input) = task_input {
                    env_vars.insert("ORK_INPUT_JSON".to_string(), input.to_string());
                }
                if let Some(name) = env_vars.get("task_name").cloned() {
                    env_vars.insert("ORK_TASK_NAME".to_string(), name);
                }
                if let Some(name) = env_vars.get("workflow_name").cloned() {
                    env_vars.insert("ORK_WORKFLOW_NAME".to_string(), name);
                }
                if let Some(run_id) = env_vars.get("run_id").cloned() {
                    env_vars.insert("ORK_RUN_ID".to_string(), run_id);
                }
                if let Some(root) = python_path.as_ref() {
                    cmd.current_dir(root);
                    env_vars.insert("PYTHONPATH".to_string(), root.to_string_lossy().to_string());
                }
            }

            for (key, value) in env_vars.iter() {
                cmd.env(key, value);
            }

            let status = match cmd.output().await {
                Ok(output) => {
                    if output.status.success() {
                        info!("Process completed successfully: {}", exec_id_clone);
                        debug!("Output: {}", String::from_utf8_lossy(&output.stdout));
                        ProcessStatus::Success
                    } else {
                        warn!("Process failed: {}", exec_id_clone);
                        warn!("Error: {}", String::from_utf8_lossy(&output.stderr));
                        ProcessStatus::Failed
                    }
                }
                Err(e) => {
                    warn!("Failed to execute process {}: {}", exec_id_clone, e);
                    ProcessStatus::Failed
                }
            };

            // Update status
            let mut states = states_clone.write().await;
            states.insert(exec_id_clone.clone(), status.clone());
            drop(states);

            // Send status update through channel if available
            let tx_guard = status_tx_clone.read().await;
            if let Some(tx) = tx_guard.as_ref() {
                let status_str = match status {
                    ProcessStatus::Running => "running",
                    ProcessStatus::Success => "success",
                    ProcessStatus::Failed => "failed",
                }
                .to_string();

                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: status_str,
                });
            }
        });

        Ok(execution_id)
    }
}

fn build_env_vars(
    params: Option<serde_json::Value>,
) -> (
    HashMap<String, String>,
    Option<PathBuf>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
    Option<PathBuf>,
    Option<PathBuf>,
) {
    let mut env_vars = HashMap::new();
    let mut task_file = None;
    let mut task_module = None;
    let mut task_function = None;
    let mut task_input = None;
    let mut runner_path = None;
    let mut python_path = None;

    if let Some(params) = params {
        if let Some(obj) = params.as_object() {
            for (k, v) in obj.iter() {
                match k.as_str() {
                    "task_file" => {
                        if let Some(path) = v.as_str() {
                            task_file = Some(PathBuf::from(path));
                        }
                    }
                    "task_input" => {
                        task_input = Some(v.clone());
                        env_vars.insert("ORK_INPUT_JSON".to_string(), v.to_string());
                    }
                    "task_module" => {
                        if let Some(module) = v.as_str() {
                            task_module = Some(module.to_string());
                        }
                    }
                    "task_function" => {
                        if let Some(function) = v.as_str() {
                            task_function = Some(function.to_string());
                        }
                    }
                    "python_path" => {
                        if let Some(path) = v.as_str() {
                            python_path = Some(PathBuf::from(path));
                        }
                    }
                    "runner_path" => {
                        if let Some(path) = v.as_str() {
                            runner_path = Some(PathBuf::from(path));
                        }
                    }
                    "env" => {
                        if let Some(env_obj) = v.as_object() {
                            for (env_key, env_val) in env_obj {
                                if let Some(val) = env_val.as_str() {
                                    env_vars.insert(env_key.clone(), val.to_string());
                                } else if env_val.is_number() || env_val.is_boolean() {
                                    env_vars.insert(env_key.clone(), env_val.to_string());
                                }
                            }
                        }
                    }
                    _ => {
                        if let Some(val) = v.as_str() {
                            env_vars.insert(k.clone(), val.to_string());
                        } else if v.is_number() || v.is_boolean() {
                            env_vars.insert(k.clone(), v.to_string());
                        }
                    }
                }
            }
        }
    }

    (
        env_vars,
        task_file,
        task_module,
        task_function,
        task_input,
        runner_path,
        python_path,
    )
}

fn resolve_command_path(working_dir: &str, command: &str) -> String {
    let command_path = Path::new(command);
    if command_path.is_absolute()
        || command.contains('/')
        || command.chars().any(|c| c.is_whitespace())
    {
        command.to_string()
    } else {
        format!("{}/{}", working_dir, command)
    }
}

fn build_python_command(task_file: &Path, python_path: Option<&PathBuf>) -> Command {
    let root = python_path
        .cloned()
        .or_else(|| find_project_root(task_file))
        .unwrap_or_else(|| PathBuf::from("."));
    if root.join("pyproject.toml").exists()
    {
        let mut cmd = Command::new("uv");
        cmd.args(["run", "python3"]);
        cmd
    } else {
        Command::new("python3")
    }
}

fn find_project_root(task_path: &Path) -> Option<PathBuf> {
    let start = task_path.parent()?;
    for ancestor in start.ancestors() {
        if ancestor.join("pyproject.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    Some(start.to_path_buf())
}

fn find_python_runner() -> Option<PathBuf> {
    let start = std::env::current_dir().ok()?;
    for ancestor in start.ancestors() {
        let candidate = ancestor.join("scripts").join("run_python_task.py");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[async_trait]
impl Executor for ProcessExecutor {
    async fn execute(
        &self,
        task_id: Uuid,
        job_name: &str,
        params: Option<serde_json::Value>,
    ) -> Result<String> {
        self.execute_process(task_id, job_name, params).await
    }

    async fn set_status_channel(&self, tx: mpsc::UnboundedSender<StatusUpdate>) {
        let mut status_tx = self.status_tx.write().await;
        *status_tx = Some(tx);
    }
}
