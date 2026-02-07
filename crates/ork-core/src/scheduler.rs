use anyhow::Result;
use futures::stream::{self, StreamExt};
use chrono::Utc;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info};
use uuid::Uuid;

use crate::config::OrchestratorConfig;

use crate::database::Database;

use crate::executor::StatusUpdate;

use crate::executor_manager::ExecutorManager;

use crate::models::{TaskStatus, Workflow, WorkflowTask, json_inner};

use crate::task_execution::{build_run_tasks, execute_task, retry_backoff_seconds};

use crate::triggerer::{JobCompletionNotification, Triggerer, TriggererConfig};
use crate::job_tracker::{BigQueryTracker, CloudRunTracker, CustomHttpTracker, DataprocTracker};

#[derive(Debug, Default, Serialize)]
pub struct SchedulerMetrics {
    #[allow(dead_code)]
    pub timestamp: u64,
    pub process_pending_runs_ms: u128,
    pub process_pending_tasks_ms: u128,
    pub process_status_updates_ms: u128,
    pub sleep_ms: u128,
    pub total_loop_ms: u128,
}

pub struct Scheduler<D: Database + 'static, E: ExecutorManager + 'static> {
    db: Arc<D>,
    config: OrchestratorConfig,
    executor_manager: Arc<E>,
    status_tx: mpsc::UnboundedSender<StatusUpdate>,
    status_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<StatusUpdate>>>,
    job_completion_tx: mpsc::UnboundedSender<JobCompletionNotification>,
    job_completion_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<JobCompletionNotification>>>,
}

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub fn new(db: Arc<D>, executor_manager: Arc<E>) -> Self {
        Self::new_with_config(db, executor_manager, OrchestratorConfig::default())
    }

    pub fn new_with_config(
        db: Arc<D>,
        executor_manager: Arc<E>,
        config: OrchestratorConfig,
    ) -> Self {
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        let (job_completion_tx, job_completion_rx) = mpsc::unbounded_channel();
        Self {
            db,
            config,
            executor_manager,
            status_tx,
            status_rx: Arc::new(tokio::sync::Mutex::new(status_rx)),
            job_completion_tx,
            job_completion_rx: Arc::new(tokio::sync::Mutex::new(job_completion_rx)),
        }
    }

    /// Start the triggerer component for tracking deferred jobs
    pub async fn start_triggerer(&self) -> Result<tokio::task::JoinHandle<()>> {
        let mut triggerer = Triggerer::with_config(
            Arc::clone(&self.db),
            self.job_completion_tx.clone(),
            TriggererConfig::default(),
        );

        // Register job trackers
        triggerer.register_tracker(Arc::new(BigQueryTracker::new().await?));
        triggerer.register_tracker(Arc::new(CloudRunTracker::new().await?));
        triggerer.register_tracker(Arc::new(DataprocTracker::new().await?));
        triggerer.register_tracker(Arc::new(CustomHttpTracker::new()));

        Ok(triggerer.start())
    }

    pub async fn run(&self) -> Result<()> {
        info!(
            "Starting scheduler loop (max_batch={}, max_concurrent_dispatch={}, max_concurrent_status={})",
            self.config.max_tasks_per_batch,
            self.config.max_concurrent_dispatches,
            self.config.max_concurrent_status_checks
        );

        // Start triggerer in background
        let _triggerer_handle = self.start_triggerer().await?;
        info!("Triggerer started");

        let mut status_rx = self.status_rx.lock().await;
        let mut job_completion_rx = self.job_completion_rx.lock().await;

        loop {
            let loop_start = Instant::now();
            let mut metrics = SchedulerMetrics::default();

            // Process scheduled triggers first
            if let Err(e) = self.process_scheduled_triggers().await { error!("Error processing scheduled triggers: {}", e); }
            // Process pending runs and tasks
            let start = Instant::now();
            let runs_processed = self.process_pending_runs().await.unwrap_or_else(|e| { error!("Error processing pending runs: {}", e); 0 });
            metrics.process_pending_runs_ms = start.elapsed().as_millis();
            let start = Instant::now();
            let tasks_processed = self.process_pending_tasks().await.unwrap_or_else(|e| { error!("Error processing pending tasks: {}", e); 0 });
            metrics.process_pending_tasks_ms = start.elapsed().as_millis();

            // Process any immediately available status updates
            let start = Instant::now();
            let mut status_updates = Vec::new();
            while let Ok(update) = status_rx.try_recv() {
                status_updates.push(update);
            }
            let updates_processed = status_updates.len();
            if !status_updates.is_empty() {
                if let Err(e) = self.process_status_updates(status_updates).await {
                    error!("Error processing status updates: {}", e);
                }
            }
            metrics.process_status_updates_ms = start.elapsed().as_millis();

            // Process deferred job completions from triggerer
            let mut job_completions = Vec::new();
            while let Ok(completion) = job_completion_rx.try_recv() {
                job_completions.push(completion);
            }
            if !job_completions.is_empty() {
                if let Err(e) = self.process_job_completions(job_completions).await {
                    error!("Error processing job completions: {}", e);
                }
            }

            if let Err(e) = self.enforce_timeouts().await {
                error!("Error enforcing timeouts: {}", e);
            }

            // Only sleep if there was no work this iteration
            // When work is processed, immediately loop back to check for more
            let had_work = runs_processed > 0 || tasks_processed > 0 || updates_processed > 0;
            let start = Instant::now();

            if !had_work {
                // No work - sleep until status update or timeout
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs_f64(self.config.poll_interval_secs)) => {
                        // Timeout - continue to next iteration
                    }
                    Some(update) = status_rx.recv() => {
                        // Got status update - process it and loop back immediately
                        if let Err(e) = self.process_status_updates(vec![update]).await {
                            error!("Error processing status update: {}", e);
                        }
                    }
                }
            }

            metrics.sleep_ms = start.elapsed().as_millis();
            metrics.total_loop_ms = loop_start.elapsed().as_millis();
            metrics.timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Output structured JSON metrics for easy parsing
            let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();
            info!("SCHEDULER_METRICS: {}", metrics_json);
        }
    }

    async fn process_pending_runs(&self) -> Result<usize> {
        let runs = self.db.get_pending_runs().await?;

        if runs.is_empty() {
            return Ok(0);
        }

        let count = runs.len();

        // Get all unique workflow IDs
        let workflow_ids: Vec<Uuid> = runs
            .iter()
            .map(|r| r.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // Batch fetch all workflows in a single query
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;

        // Create workflow lookup map
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();

        let mut workflow_tasks_map: HashMap<Uuid, Vec<WorkflowTask>> = HashMap::new();
        for workflow_id in workflow_map.keys() {
            let tasks = self.db.list_workflow_tasks(*workflow_id).await?;
            if !tasks.is_empty() {
                workflow_tasks_map.insert(*workflow_id, tasks);
            }
        }

        // Create tasks first, then update runs to running
        // This ensures runs are only marked as running if tasks were successfully created
        for run in &runs {
            if let Some(workflow) = workflow_map.get(&run.workflow_id) {
                let create_result =
                    if let Some(workflow_tasks) = workflow_tasks_map.get(&run.workflow_id) {
                        let tasks = build_run_tasks(run.id, workflow, workflow_tasks);
                        self.db.batch_create_dag_tasks(run.id, &tasks).await
                    } else if workflow.executor_type == "dag" {
                        Err(anyhow::anyhow!(
                            "workflow '{}' is missing compiled workflow_tasks",
                            workflow.name
                        ))
                    } else {
                        let task_count = workflow
                            .task_params
                            .as_ref()
                            .and_then(|p| json_inner(p).get("task_count"))
                            .and_then(|v: &serde_json::Value| v.as_i64())
                            .unwrap_or(3) as i32;
                        self.db
                            .batch_create_tasks(
                                run.id,
                                task_count,
                                &workflow.name,
                                &workflow.executor_type,
                            )
                            .await
                    };

                // Create tasks
                if let Err(e) = create_result {
                    error!("Failed to create tasks for run {}: {}", run.id, e);
                    // Mark run as failed
                    let _ = self
                        .db
                        .update_run_status(run.id, "failed", Some(&e.to_string()))
                        .await;
                    continue;
                }

                // Tasks created successfully - update run to running
                if let Err(e) = self.db.update_run_status(run.id, "running", None).await {
                    error!("Failed to update run {} to running: {}", run.id, e);
                }
            }
        }

        Ok(count)
    }

    async fn process_pending_tasks(&self) -> Result<usize> {
        // OPTIMIZATION: Single JOIN query instead of N+1 queries
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(0);
        }

        let count = tasks.len();

        let workflow_ids: Vec<Uuid> = tasks
            .iter()
            .map(|task| task.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();
        let workflow_map = Arc::new(workflow_map);

        let mut deps_by_run: HashMap<Uuid, HashSet<String>> = HashMap::new();
        for task in &tasks {
            if task.depends_on.is_empty() {
                continue;
            }
            let deps = deps_by_run.entry(task.run_id).or_default();
            for name in &task.depends_on {
                deps.insert(name.clone());
            }
        }

        let mut outputs_by_run: HashMap<Uuid, HashMap<String, serde_json::Value>> = HashMap::new();
        for (run_id, deps) in deps_by_run {
            let names: Vec<String> = deps.into_iter().collect();
            match self.db.get_task_outputs(run_id, &names).await {
                Ok(map) => {
                    outputs_by_run.insert(run_id, map);
                }
                Err(err) => {
                    error!("Failed to load upstream outputs for run {}: {}", run_id, err);
                }
            }
        }
        let outputs_by_run = Arc::new(outputs_by_run);

        // OPTIMIZATION: Process tasks concurrently with limit to avoid memory explosion
        let results = stream::iter(tasks)
            .map(|task_with_workflow| {
                let executor_manager = Arc::clone(&self.executor_manager);
                let status_tx = self.status_tx.clone();
                let workflow_map = workflow_map.clone();
                let outputs_by_run = outputs_by_run.clone();

                async move {
                    execute_task(
                        task_with_workflow,
                        workflow_map,
                        outputs_by_run,
                        &*executor_manager,
                        status_tx,
                    )
                    .await
                }
            })
            .buffer_unordered(self.config.max_concurrent_dispatches)
            .collect::<Vec<_>>()
            .await;

        let mut dispatch_updates: Vec<(Uuid, String)> = Vec::new();
        let mut failure_updates: Vec<StatusUpdate> = Vec::new();

        for (task_id, result) in results {
            match result {
                Ok(execution_name) => {
                    dispatch_updates.push((task_id, execution_name));
                }
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task_id, e);
                    failure_updates.push(StatusUpdate {
                        task_id,
                        status: "failed".to_string(),
                        log: None,
                        output: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let db_update_start = Instant::now();
        if !dispatch_updates.is_empty() {
            let updates_ref: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = dispatch_updates
                .iter()
                .map(|(id, execution)| (*id, "dispatched", Some(execution.as_str()), None))
                .collect();
            self.db.batch_update_task_status(&updates_ref).await?;
        }
        let db_update_ms = db_update_start.elapsed().as_millis();
        info!(
            "Batch updated {} tasks in {}ms",
            dispatch_updates.len(),
            db_update_ms
        );

        if !failure_updates.is_empty() {
            self.process_status_updates(failure_updates).await?;
        }

        Ok(count)
    }

    async fn process_status_updates(&self, updates: Vec<StatusUpdate>) -> Result<()> {
        let mut task_updates: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = Vec::new();
        let mut runs_to_check = std::collections::HashSet::new();
        let mut failed_by_run: HashMap<Uuid, Vec<String>> = HashMap::new();
        let failed_ids: Vec<Uuid> = updates.iter().filter(|u| u.status == "failed").map(|u| u.task_id).collect();
        let retry_meta = if failed_ids.is_empty() {
            HashMap::new()
        } else {
            self.db.get_task_retry_meta(&failed_ids).await?
        };

        for update in &updates {
            if let Some(log) = update.log.as_ref() {
                if let Err(e) = self.db.append_task_log(update.task_id, log).await {
                    error!("Failed to append log for task {}: {}", update.task_id, e);
                }
            }
            if let Some(output) = update.output.as_ref() {
                // Check if output contains deferrables
                if let Some(deferred) = output.get("deferred").and_then(|v| v.as_array()) {
                    // Task returned deferrables - create deferred jobs
                    info!("Task {} returned {} deferrables", update.task_id, deferred.len());

                    for deferrable_data in deferred {
                        if let (Some(service_type), Some(job_id)) = (
                            deferrable_data.get("service_type").and_then(|v| v.as_str()),
                            deferrable_data.get("job_id").and_then(|v| v.as_str()),
                        ) {
                            match self.db.create_deferred_job(
                                update.task_id,
                                service_type,
                                job_id,
                                deferrable_data.clone(),
                            ).await {
                                Ok(_) => {
                                    info!("Created deferred job for task {}: {} ({})", update.task_id, job_id, service_type);
                                }
                                Err(e) => {
                                    error!("Failed to create deferred job for task {}: {}", update.task_id, e);
                                }
                            }
                        } else {
                            error!("Invalid deferrable data for task {}: missing service_type or job_id", update.task_id);
                        }
                    }

                    // Don't mark task as success yet - it will be marked when deferred jobs complete
                    // Update task to a "deferred" status (we'll treat it as running)
                    continue;
                }

                if let Err(e) = self.db.update_task_output(update.task_id, output.clone()).await {
                    error!("Failed to store output for task {}: {}", update.task_id, e);
                }
            }

            let new_status = match update.status.as_str() {
                "success" => TaskStatus::Success,
                "failed" => TaskStatus::Failed,
                "running" => TaskStatus::Running,
                _ => continue,
            };

            if matches!(new_status, TaskStatus::Failed) {
                let (attempts, max_retries) = retry_meta.get(&update.task_id).cloned().unwrap_or((0, 0));
                if attempts < max_retries {
                    let error = update.error.as_deref().unwrap_or("retrying");
                    let backoff = retry_backoff_seconds(attempts + 1);
                    let retry_at = Utc::now() + chrono::Duration::seconds(i64::try_from(backoff).unwrap_or(i64::MAX));
                    if let Err(e) = self.db.reset_task_for_retry(update.task_id, Some(error), Some(retry_at)).await {
                        error!("Failed to reset task {} for retry: {}", update.task_id, e);
                        task_updates.push((update.task_id, new_status.as_str(), None, update.error.as_deref()));
                    } else {
                        continue;
                    }
                }
            }

            task_updates.push((
                update.task_id,
                new_status.as_str(),
                None,
                update.error.as_deref(),
            ));

            if matches!(new_status, TaskStatus::Success | TaskStatus::Failed) {
                let (run_id, task_name) = self.db.get_task_identity(update.task_id).await?;
                runs_to_check.insert(run_id);
                if matches!(new_status, TaskStatus::Failed) {
                    failed_by_run.entry(run_id).or_default().push(task_name);
                }
            }
        }

        if !task_updates.is_empty() {
            self.db.batch_update_task_status(&task_updates).await?;
            info!(
                "Processed {} status updates from executors",
                task_updates.len()
            );
        }

        // Propagate dependency failures to downstream tasks
        for (run_id, names) in failed_by_run {
            let mut pending = names;
            let mut seen = HashSet::new();
            while !pending.is_empty() {
                let next = self
                    .db
                    .mark_tasks_failed_by_dependency(run_id, &pending, "dependency failed")
                    .await?;
                pending = next.into_iter().filter(|name| seen.insert(name.clone())).collect();
            }
            runs_to_check.insert(run_id);
        }

        for run_id in runs_to_check {
            self.check_run_completion(run_id).await?;
        }

        Ok(())
    }

    async fn enforce_timeouts(&self) -> Result<()> {
        let running = self.db.get_running_tasks().await?;
        if running.is_empty() {
            return Ok(());
        }

        let now = Utc::now();
        let mut updates = Vec::new();
        for task in running {
            let timeout = match task.timeout_seconds {
                Some(value) if value > 0 => value,
                _ => continue,
            };
            let started = task
                .started_at
                .or(task.dispatched_at)
                .unwrap_or(task.created_at);
            let elapsed = now.signed_duration_since(started).num_seconds();
            if elapsed > i64::from(timeout) {
                updates.push(StatusUpdate {
                    task_id: task.id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(format!("timeout after {}s", timeout)),
                });
            }
        }

        if !updates.is_empty() {
            self.process_status_updates(updates).await?;
        }

        Ok(())
    }

    async fn check_run_completion(&self, run_id: Uuid) -> Result<()> {
        // Use a COUNT query instead of fetching all tasks
        let (total, completed, failed) = self.db.get_run_task_stats(run_id).await?;

        if completed == total {
            let new_status = if failed > 0 {
                TaskStatus::Failed
            } else {
                TaskStatus::Success
            };

            self.db
                .update_run_status(run_id, new_status.as_str(), None)
                .await?;
            info!(
                "Run {} completed with status: {} ({}/{} tasks)",
                run_id,
                new_status.as_str(),
                completed,
                total
            );
        }

        Ok(())
    }

    async fn process_scheduled_triggers(&self) -> Result<usize> {
        crate::schedule_processor::process_scheduled_triggers(self.db.as_ref()).await
    }

    /// Process job completion notifications from the triggerer
    async fn process_job_completions(
        &self,
        completions: Vec<JobCompletionNotification>,
    ) -> Result<()> {
        for completion in completions {
            info!(
                "Processing deferred job completion for task {}: success={}",
                completion.task_id, completion.success
            );

            // Update task status based on job completion
            if completion.success {
                // Mark task as successful
                if let Err(e) = self
                    .db
                    .update_task_status(completion.task_id, "success", None, None)
                    .await
                {
                    error!(
                        "Failed to mark task {} as successful: {}",
                        completion.task_id, e
                    );
                    continue;
                }
            } else {
                // Mark task as failed with error
                let error_msg = completion
                    .error
                    .unwrap_or_else(|| "Deferred job failed".to_string());
                if let Err(e) = self
                    .db
                    .update_task_status(completion.task_id, "failed", None, Some(&error_msg))
                    .await
                {
                    error!("Failed to mark task {} as failed: {}", completion.task_id, e);
                    continue;
                }
            }

            // Check if the run should be marked as complete
            if let Ok(run_id) = self.db.get_task_run_id(completion.task_id).await {
                if let Err(e) = self.check_run_completion(run_id).await {
                    error!("Error checking run completion: {}", e);
                }
            }
        }

        Ok(())
    }
}
