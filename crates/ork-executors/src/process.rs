use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock, mpsc};
use tracing::{info, warn};
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

        let start_msg = format!(
            "Executing process: {} with execution_id: {}",
            command, execution_id
        );
        info!("{}", start_msg);

        let (
            mut env_vars,
            task_file,
            task_module,
            task_function,
            task_input,
            runner_path,
            python_path,
        ) = build_env_vars(params);
        let task_label = env_vars
            .get("task_name")
            .cloned()
            .unwrap_or_else(|| task_id.to_string());

        // Add execution ID to env
        env_vars.insert("EXECUTION_ID".to_string(), execution_id.clone());

        // Mark as running
        {
            let mut states = self.process_states.write().await;
            states.insert(execution_id.clone(), ProcessStatus::Running);
        }

        // Clone for the async task
        let exec_id_clone = execution_id.clone();
        let task_label_clone = task_label.clone();
        let states_clone = self.process_states.clone();
        let status_tx_clone = self.status_tx.clone();
        let last_output: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let working_dir = self.working_dir.clone();
        let command = command.clone();

        tokio::spawn(async move {
            let (mut cmd, uses_uv) = if let Some(task_file) = task_file.as_ref() {
                let (mut python_cmd, uses_uv) =
                    build_python_command(task_file, python_path.as_ref());
                let runner = runner_path
                    .or_else(|| crate::python_runtime::get_run_task_script().ok())
                    .unwrap_or_else(|| PathBuf::from("run_python_task.py"));
                python_cmd.arg(runner);
                (python_cmd, uses_uv)
            } else if task_module.is_some() {
                let (mut python_cmd, uses_uv) =
                    build_python_command(Path::new("."), python_path.as_ref());
                let runner = runner_path
                    .or_else(|| crate::python_runtime::get_run_task_script().ok())
                    .unwrap_or_else(|| PathBuf::from("run_python_task.py"));
                python_cmd.arg(runner);
                (python_cmd, uses_uv)
            } else {
                let command_path = resolve_command_path(&working_dir, &command);
                let mut shell_cmd = Command::new("sh");
                shell_cmd.arg("-c").arg(&command_path);
                shell_cmd.current_dir(&working_dir);
                (shell_cmd, false)
            };

            if let Some(task_file) = task_file.as_ref() {
                env_vars.insert(
                    "ORK_TASK_FILE".to_string(),
                    task_file.to_string_lossy().to_string(),
                );
                if let Some(input) = task_input {
                    env_vars.insert("ORK_INPUT_JSON".to_string(), input.to_string());
                }
                if let Some(function) = task_function.as_ref() {
                    env_vars.insert("ORK_TASK_FUNCTION".to_string(), function.to_string());
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
                if let Some(root) = python_path
                    .as_ref()
                    .cloned()
                    .or_else(|| find_project_root(task_file))
                {
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

            if uses_uv && !env_vars.contains_key("UV_CACHE_DIR") {
                let cache_dir = std::env::var("XDG_CACHE_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| std::env::temp_dir())
                    .join("ork-uv-cache");
                let _ = std::fs::create_dir_all(&cache_dir);
                env_vars.insert(
                    "UV_CACHE_DIR".to_string(),
                    cache_dir.to_string_lossy().to_string(),
                );
            }

            for (key, value) in env_vars.iter() {
                cmd.env(key, value);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let error_msg = format!(
                        "failed to spawn process for task {} (execution {}): {}",
                        task_label_clone, exec_id_clone, e
                    );
                    let warn_msg = format!(
                        "Failed to execute task {} (execution {}): {}",
                        task_label_clone, exec_id_clone, e
                    );
                    warn!("{}", warn_msg);
                    let mut states = states_clone.write().await;
                    states.insert(exec_id_clone.clone(), ProcessStatus::Failed);
                    drop(states);
                    if let Some(tx) = status_tx_clone.read().await.as_ref() {
                        let _ = tx.send(StatusUpdate {
                            task_id,
                            status: "failed".to_string(),
                            log: None,
                            output: None,
                            error: Some(error_msg),
                        });
                    }
                    return;
                }
            };

            if let Some(tx) = status_tx_clone.read().await.as_ref() {
                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: "running".to_string(),
                    log: None,
                    output: None,
                    error: None,
                });
            }

            let stdout_reader = child.stdout.take().map(BufReader::new);
            let stderr_reader = child.stderr.take().map(BufReader::new);

            let status_tx_logs = status_tx_clone.clone();
            let log_task_id = task_id;
            let output_store = last_output.clone();
            let stdout_handle = tokio::spawn(async move {
                let mut reader = stdout_reader.expect("stdout must be piped");
                let mut line = String::new();
                loop {
                    line.clear();
                    let bytes_read = reader.read_line(&mut line).await.unwrap_or_default();
                    if bytes_read == 0 {
                        break;
                    }
                    let trimmed = line.trim_end();
                    // Check for output prefix to distinguish from debug prints
                    if let Some(json_str) = trimmed.strip_prefix("ORK_OUTPUT:")
                        && let Ok(value) =
                            serde_json::from_str::<serde_json::Value>(json_str.trim())
                    {
                        *output_store.lock().await = Some(value);
                    }
                    // Send all stdout lines as logs
                    if let Some(tx) = status_tx_logs.read().await.as_ref() {
                        let _ = tx.send(StatusUpdate {
                            task_id: log_task_id,
                            status: "log".to_string(),
                            log: Some(format!("stdout: {}\n", trimmed)),
                            output: None,
                            error: None,
                        });
                    }
                }
            });

            let status_tx_logs = status_tx_clone.clone();
            let log_task_id = task_id;
            let stderr_handle = tokio::spawn(async move {
                let mut reader = stderr_reader.expect("stderr must be piped");
                let mut line = String::new();
                loop {
                    line.clear();
                    let bytes_read = reader.read_line(&mut line).await.unwrap_or_default();
                    if bytes_read == 0 {
                        break;
                    }
                    let trimmed = line.trim_end();
                    if let Some(tx) = status_tx_logs.read().await.as_ref() {
                        let _ = tx.send(StatusUpdate {
                            task_id: log_task_id,
                            status: "log".to_string(),
                            log: Some(format!("stderr: {}\n", trimmed)),
                            output: None,
                            error: None,
                        });
                    }
                }
            });

            let status = child
                .wait()
                .await
                .ok()
                .map_or(ProcessStatus::Failed, |exit_status| {
                    if exit_status.success() {
                        info!("Process completed successfully: {}", exec_id_clone);
                        ProcessStatus::Success
                    } else {
                        let warn_msg = format!(
                            "Process failed for task {}: {}",
                            task_label_clone, exec_id_clone
                        );
                        warn!("{}", warn_msg);
                        ProcessStatus::Failed
                    }
                });

            let _ = stdout_handle.await;
            let _ = stderr_handle.await;

            // Update status
            let mut states = states_clone.write().await;
            states.insert(exec_id_clone.clone(), status.clone());
            drop(states);

            // Send status update through channel if available
            let tx_guard = status_tx_clone.read().await;
            if let Some(tx) = tx_guard.as_ref() {
                let status_str = if matches!(status, ProcessStatus::Success) {
                    "success"
                } else {
                    "failed"
                }
                .to_string();

                let output = if matches!(status, ProcessStatus::Success) {
                    last_output.lock().await.clone()
                } else {
                    None
                };
                let error = if matches!(status, ProcessStatus::Failed) {
                    Some(format!(
                        "process failed for task {} (execution {})",
                        task_label_clone, exec_id_clone
                    ))
                } else {
                    None
                };

                let _ = tx.send(StatusUpdate {
                    task_id,
                    status: status_str,
                    log: None,
                    output,
                    error,
                });
            }
        });

        Ok(execution_id)
    }
}

type ProcessEnvBuild = (
    HashMap<String, String>,
    Option<PathBuf>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
    Option<PathBuf>,
    Option<PathBuf>,
);

fn build_env_vars(params: Option<serde_json::Value>) -> ProcessEnvBuild {
    let mut env_vars = HashMap::new();
    let mut task_file = None;
    let mut task_module = None;
    let mut task_function = None;
    let mut task_input = None;
    let mut runner_path = None;
    let mut python_path = None;

    if let Some(params) = params
        && let Some(obj) = params.as_object()
    {
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
                "upstream" => {
                    env_vars.insert("ORK_UPSTREAM_JSON".to_string(), v.to_string());
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

fn resolve_command_path(_working_dir: &str, command: &str) -> String {
    let command_path = Path::new(command);
    if command_path.is_absolute() || command.chars().any(|c| c.is_whitespace()) {
        return command.to_string();
    }

    if command.contains('/') {
        return command.to_string();
    }

    format!("./{}", command)
}

fn build_python_command(task_file: &Path, python_path: Option<&PathBuf>) -> (Command, bool) {
    let root = python_path
        .cloned()
        .or_else(|| find_project_root(task_file))
        .unwrap_or_else(|| PathBuf::from("."));
    if root.join("pyproject.toml").exists() {
        let mut cmd = Command::new("uv");
        cmd.args(["run", "python3"]);
        (cmd, true)
    } else {
        (Command::new("python3"), false)
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

// Python runner is now provided by crate::python_runtime::get_run_task_script()

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

#[cfg(test)]
mod tests;
