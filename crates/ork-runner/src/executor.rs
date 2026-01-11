use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use indexmap::IndexMap;
use tokio::{
    fs,
    process::Command,
    time::{sleep, timeout},
};

use ork_core::compiled::CompiledTask;
use ork_core::error::{OrkError, OrkResult};

const MIN_TASK_DURATION: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct TaskContext {
    pub workflow_name: String,
    pub task_name: String,
    pub run_id: String,
    pub attempt: u32,
    pub task_dir: PathBuf,
    pub spec_path: PathBuf,
    pub output_path: PathBuf,
}

#[derive(Debug)]
pub struct TaskExecutionResult {
    pub status: TaskExecutionStatus,
    pub stdout: String,
    pub stderr: String,
    pub output: Option<serde_json::Value>,
    pub log_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskExecutionStatus {
    Success,
    Failed(String),
    TimedOut,
}

pub async fn execute_task(
    task: &CompiledTask,
    _upstream: &IndexMap<String, serde_json::Value>,
    context: &TaskContext,
) -> OrkResult<TaskExecutionResult> {
    let started_at = Instant::now();
    fs::create_dir_all(&context.task_dir)
        .await
        .map_err(|source| OrkError::WriteFile {
            path: context.task_dir.clone(),
            source,
        })?;

    let log_path = context.task_dir.join("log.txt");
    let file_path = resolve_task_file(&task.file);
    let workdir = file_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let project_root = find_project_root(&workdir);
    let command = build_command(task, context, &file_path, &project_root);

    let status = run_with_timeout(
        command,
        Duration::from_secs(task.timeout),
        &context.task_name,
    )
    .await;

    let (stdout, stderr, status) = match status {
        Ok(output) => map_process_output(output),
        Err(OrkError::Timeout { .. }) => {
            sleep(Duration::from_millis(50)).await;
            (String::new(), String::new(), TaskExecutionStatus::TimedOut)
        }
        Err(err) => {
            ensure_min_runtime(started_at).await;
            return Err(err);
        }
    };

    let output_json = read_output_json(&context.output_path, &stdout).await?;
    write_logs(&log_path, &stdout, &stderr).await?;

    ensure_min_runtime(started_at).await;

    Ok(TaskExecutionResult {
        status,
        stdout,
        stderr,
        output: output_json,
        log_path,
    })
}

async fn ensure_min_runtime(started_at: Instant) {
    let elapsed = started_at.elapsed();
    if elapsed < MIN_TASK_DURATION {
        sleep(MIN_TASK_DURATION - elapsed).await;
    }
}

fn build_command(
    task: &CompiledTask,
    context: &TaskContext,
    file_path: &Path,
    project_root: &Path,
) -> Command {
    let mut command = if has_pyproject(project_root) {
        let mut cmd = Command::new("uv");
        cmd.args(["run", "python3"]).arg(&file_path);
        cmd
    } else {
        let mut cmd = Command::new("python3");
        cmd.arg(&file_path);
        cmd
    };

    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(&task.env)
        .env("PYTHONPATH", project_root)
        .env("ORK_INPUT_PATH", &context.spec_path)
        .env("ORK_OUTPUT_PATH", &context.output_path)
        .env("ORK_TASK_NAME", &context.task_name)
        .env("ORK_WORKFLOW_NAME", &context.workflow_name)
        .env("ORK_RUN_ID", &context.run_id)
        .env("ORK_ATTEMPT", context.attempt.to_string())
        .env("ORK_WORKDIR", &context.task_dir)
        .current_dir(project_root);
    command.kill_on_drop(true);
    command
}

fn has_pyproject(dir: &Path) -> bool {
    dir.join("pyproject.toml").exists()
}

fn find_project_root(start: &Path) -> PathBuf {
    for ancestor in start.ancestors() {
        if has_pyproject(ancestor) {
            return ancestor.to_path_buf();
        }
    }
    start.to_path_buf()
}

fn resolve_task_file(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

fn map_process_output(output: std::process::Output) -> (String, String, TaskExecutionStatus) {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status = if output.status.success() {
        TaskExecutionStatus::Success
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".into());
        TaskExecutionStatus::Failed(format!("process exited with status {}", code))
    };
    (stdout, stderr, status)
}

async fn read_output_json(path: &Path, stdout: &str) -> OrkResult<Option<serde_json::Value>> {
    if path.exists() {
        let data = fs::read(path).await.map_err(|source| OrkError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        if !data.is_empty() {
            return serde_json::from_slice(&data)
                .map(Some)
                .map_err(|source| OrkError::JsonParse {
                    path: path.to_path_buf(),
                    source,
                });
        }
        return Ok(None);
    }

    if stdout.trim().is_empty() {
        return Ok(None);
    }

    match serde_json::from_str::<serde_json::Value>(stdout) {
        Ok(val) => Ok(Some(val)),
        Err(_) => Ok(None),
    }
}

async fn write_logs(path: &Path, stdout: &str, stderr: &str) -> OrkResult<()> {
    if stdout.is_empty() && stderr.is_empty() {
        return Ok(());
    }

    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr);
    }

    fs::write(path, combined)
        .await
        .map_err(|source| OrkError::WriteFile {
            path: path.to_path_buf(),
            source,
        })
}

async fn run_with_timeout(
    mut command: Command,
    timeout_duration: Duration,
    task_name: &str,
) -> OrkResult<std::process::Output> {
    let child = command.spawn().map_err(|source| OrkError::ExecutorError {
        task: task_name.to_string(),
        source,
    })?;

    let output = timeout(timeout_duration, child.wait_with_output()).await;
    output
        .map_err(|_| OrkError::Timeout {
            task: task_name.to_string(),
            seconds: timeout_duration.as_secs(),
        })?
        .map_err(|source| OrkError::ExecutorError {
            task: task_name.to_string(),
            source,
        })
}
