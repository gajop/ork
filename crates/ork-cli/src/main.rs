mod cli;

use std::path::{Path, PathBuf};

use chrono::Utc;
use clap::{Parser, ValueEnum};
use cli::{Cli, Commands};
use ork_core::error::OrkResult;
use ork_core::types::{Run, TaskRun, TaskStatus};
use ork_core::workflow::Workflow;
use ork_runner::{LocalScheduler, LocalTaskState, RunSummary};
use ork_state::{FileStateStore, InMemoryStateStore, LocalObjectStore, ObjectStore};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum StateBackend {
    File,
    Memory,
}

fn build_state_store(
    backend: StateBackend,
    state_base: &Path,
) -> std::sync::Arc<dyn ork_state::StateStore> {
    match backend {
        StateBackend::File => std::sync::Arc::new(FileStateStore::new(state_base)),
        StateBackend::Memory => std::sync::Arc::new(InMemoryStateStore::default()),
    }
}

#[tokio::main]
async fn main() -> OrkResult<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { workflow } => {
            let wf = Workflow::load(&workflow)?;
            println!(
                "Workflow `{}` is valid with {} task(s)",
                wf.name,
                wf.tasks.len()
            );
        }
        Commands::Run {
            workflow,
            parallel,
            run_dir,
            state_dir,
            state_backend,
        } => handle_run(workflow, parallel, run_dir, state_dir, state_backend).await?,
        Commands::Status {
            run_id,
            run_dir,
            state_dir,
            limit,
            show_outputs,
            state_backend,
        } => {
            handle_status(
                run_id,
                run_dir,
                state_dir,
                limit,
                show_outputs,
                state_backend,
            )
            .await?
        }
    }

    Ok(())
}

async fn handle_run(
    workflow: PathBuf,
    parallel: usize,
    run_dir: Option<PathBuf>,
    state_dir: Option<PathBuf>,
    state_backend: StateBackend,
) -> OrkResult<()> {
    let wf = Workflow::load(&workflow)?;
    let root = workflow
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let compiled = wf.compile(root)?;
    let base_dir = run_dir.unwrap_or_else(|| PathBuf::from(".ork/runs"));
    let state_base = state_dir.unwrap_or_else(|| PathBuf::from(".ork/state"));
    let state = build_state_store(state_backend, &state_base);
    let object_store = std::sync::Arc::new(LocalObjectStore::new(&base_dir));
    let scheduler = LocalScheduler::with_state(state, object_store.clone(), &base_dir, parallel);
    let summary = scheduler.run_compiled(wf.clone(), compiled).await?;
    let stored = scheduler
        .task_runs(&summary.run_id)
        .await
        .unwrap_or_default();
    let run_record = scheduler.get_run(&summary.run_id).await.unwrap_or(None);
    let store = object_store;
    let mut stored_outputs = Vec::new();
    for task in &stored {
        let _spec: Option<ork_core::types::TaskSpec> =
            store.read_spec(&summary.run_id, &task.task).await.ok();
        let output = store
            .read_output(&summary.run_id, &task.task)
            .await
            .ok()
            .flatten();
        let status_file = store
            .read_status(&summary.run_id, &task.task)
            .await
            .ok()
            .flatten();
        stored_outputs.push((task.task.clone(), output, status_file.map(|s| s.status)));
    }
    print_summary(&wf.name, &summary, &stored, run_record, stored_outputs);
    Ok(())
}

async fn handle_status(
    run_id: Option<String>,
    run_dir: Option<PathBuf>,
    state_dir: Option<PathBuf>,
    limit: usize,
    show_outputs: bool,
    state_backend: StateBackend,
) -> OrkResult<()> {
    let base_dir = run_dir.unwrap_or_else(|| PathBuf::from(".ork/runs"));
    let state_base = state_dir.unwrap_or_else(|| PathBuf::from(".ork/state"));
    let state = build_state_store(state_backend, &state_base);
    let object_store = std::sync::Arc::new(LocalObjectStore::new(&base_dir));
    let scheduler = LocalScheduler::with_state(state, object_store.clone(), &base_dir, 4);

    let mut runs = scheduler.list_runs().await.unwrap_or_default();
    if runs.is_empty() {
        println!("No runs found in state store.");
        return Ok(());
    }

    if run_id.is_none() {
        println!("Runs in state store (newest first):");
        runs.truncate(limit);
        for run in runs.iter() {
            println!(
                "  {} workflow={} status={:?} created_at={} started_at={:?} finished_at={:?} duration={}",
                run.id,
                run.workflow,
                run.status,
                fmt_ts(Some(run.created_at)),
                fmt_ts(run.started_at),
                fmt_ts(run.finished_at),
                fmt_duration(run.started_at, run.finished_at)
            );
        }
    }

    let chosen = run_id
        .as_ref()
        .map(|s| s.as_str())
        .or_else(|| runs.first().map(|r| r.id.as_str()))
        .and_then(|id| runs.iter().find(|r| r.id == id).cloned());

    if let Some(run) = chosen {
        println!("Run {} status: {:?}", run.id, run.status);
        let tasks = scheduler.task_runs(&run.id).await.unwrap_or_default();
        let store = object_store;
        for task in tasks {
            let status_file = store.read_status(&run.id, &task.task).await.ok().flatten();
            let output_snippet = if show_outputs {
                store
                    .read_output(&run.id, &task.task)
                    .await
                    .ok()
                    .flatten()
                    .map(|o| truncate(&o))
            } else {
                None
            };
            let duration = fmt_duration(task.started_at, task.finished_at);
            println!(
                "  {:<20} status={:?} recorded={:?} started_at={:?} finished_at={:?} duration={}{}",
                task.task,
                task.status,
                status_file.as_ref().map(|s| s.status.clone()),
                fmt_ts(task.started_at),
                fmt_ts(task.finished_at),
                duration,
                output_snippet
                    .map(|o| format!(" output={o}"))
                    .unwrap_or_default()
            );
        }
    } else if let Some(id) = run_id {
        println!("Run {} not found in state store", id);
    }
    Ok(())
}

fn print_summary(
    workflow_name: &str,
    summary: &RunSummary,
    stored: &[TaskRun],
    run_record: Option<Run>,
    stored_outputs: Vec<(String, Option<serde_json::Value>, Option<TaskStatus>)>,
) {
    println!(
        "Workflow `{}` run {} (artifacts: {})",
        workflow_name,
        summary.run_id,
        summary.run_dir.display()
    );

    for (task, status) in &summary.statuses {
        match status {
            LocalTaskState::Success { attempt, output } => {
                println!(
                    "  {:<20} ✅  attempt {}{}",
                    task,
                    attempt,
                    output
                        .as_ref()
                        .map(|o| format!(" output={}", truncate(o)))
                        .unwrap_or_default()
                );
            }
            LocalTaskState::Failed { attempt, error } => {
                println!("  {:<20} ❌  attempt {} error={}", task, attempt, error);
            }
            LocalTaskState::Running { attempt } => {
                println!("  {:<20} ⏳  running (attempt {})", task, attempt);
            }
            LocalTaskState::Pending => {
                println!("  {:<20} ⏳  pending", task);
            }
            LocalTaskState::Skipped { reason } => {
                println!("  {:<20} ➖  skipped ({reason})", task);
            }
        }
    }

    let duration = (summary.finished_at - summary.started_at).as_secs_f32();
    let timestamp = Utc::now().to_rfc3339();
    println!("Completed at {} in {:.2}s", timestamp, duration);

    if !stored.is_empty() {
        println!("State store snapshot:");
        for task in stored {
            println!(
                "  {:<20} status={:?} attempt={}",
                task.task, task.status, task.attempt
            );
        }
    }

    if let Some(run) = run_record {
        println!("Persisted run status: {:?}", run.status);
    }

    if !stored_outputs.is_empty() {
        println!("Stored task outputs:");
        for (task, output, status) in stored_outputs {
            println!(
                "  {:<20} status={:?} output={}",
                task,
                status.unwrap_or(TaskStatus::Pending),
                output
                    .map(|o| truncate(&o))
                    .unwrap_or_else(|| "<none>".into())
            );
        }
    }
}

fn truncate(value: &serde_json::Value) -> String {
    let s = value.to_string();
    if s.len() > 80 {
        format!("{}...", &s[..80])
    } else {
        s
    }
}

fn fmt_duration(
    start: Option<chrono::DateTime<Utc>>,
    end: Option<chrono::DateTime<Utc>>,
) -> String {
    match (start, end) {
        (Some(s), Some(e)) => {
            let secs = (e - s).num_milliseconds() as f64 / 1000.0;
            format!("{:.2}s", secs)
        }
        _ => "-".into(),
    }
}

fn fmt_ts(ts: Option<chrono::DateTime<Utc>>) -> String {
    ts.map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "-".into())
}
