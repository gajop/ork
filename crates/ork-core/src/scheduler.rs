use anyhow::Result;
use chrono::Utc;
use futures::stream::{self, StreamExt};
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

use crate::job_tracker::{BigQueryTracker, CloudRunTracker, CustomHttpTracker, DataprocTracker};
use crate::triggerer::{JobCompletionNotification, Triggerer, TriggererConfig};

mod processing;

#[derive(Debug, Default, Serialize)]
pub struct SchedulerMetrics {
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

    /// Enqueue an executor status update for processing in the scheduler loop.
    pub fn enqueue_status_update(&self, update: StatusUpdate) {
        let _ = self.status_tx.send(update);
    }

    /// Enqueue a deferred-job completion event for processing in the scheduler loop.
    pub fn enqueue_job_completion(&self, completion: JobCompletionNotification) {
        let _ = self.job_completion_tx.send(completion);
    }

    /// Start the triggerer component for tracking deferred jobs
    pub async fn start_triggerer(&self) -> Result<tokio::task::JoinHandle<()>> {
        // rustls 0.23 requires explicit process-level provider selection when
        // multiple providers are enabled transitively.
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

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

        // Start triggerer in background if enabled
        let _triggerer_handle = if self.config.enable_triggerer {
            let handle = self.start_triggerer().await.ok();
            if handle.is_none() {
                error!("Triggerer failed to start; continuing with scheduler-only mode");
            }
            handle
        } else {
            info!("Triggerer disabled by config");
            None
        };

        let mut status_rx = self.status_rx.lock().await;
        let mut job_completion_rx = self.job_completion_rx.lock().await;

        loop {
            let loop_start = Instant::now();
            let mut metrics = SchedulerMetrics::default();

            // Process scheduled triggers first
            if let Err(e) = self.process_scheduled_triggers().await {
                error!("Error processing scheduled triggers: {}", e);
            }
            // Process pending runs and tasks
            let start = Instant::now();
            let runs_processed = self.process_pending_runs().await.unwrap_or_else(|e| {
                error!("Error processing pending runs: {}", e);
                0
            });
            metrics.process_pending_runs_ms = start.elapsed().as_millis();
            let start = Instant::now();
            let tasks_processed = self.process_pending_tasks().await.unwrap_or_else(|e| {
                error!("Error processing pending tasks: {}", e);
                0
            });
            metrics.process_pending_tasks_ms = start.elapsed().as_millis();

            // Process any immediately available status updates
            let start = Instant::now();
            let mut status_updates = Vec::new();
            while let Ok(update) = status_rx.try_recv() {
                status_updates.push(update);
            }
            let updates_processed = status_updates.len();
            if !status_updates.is_empty()
                && let Err(e) = self.process_status_updates(status_updates).await
            {
                error!("Error processing status updates: {}", e);
            }
            metrics.process_status_updates_ms = start.elapsed().as_millis();

            // Process deferred job completions from triggerer
            let mut job_completions = Vec::new();
            while let Ok(completion) = job_completion_rx.try_recv() {
                job_completions.push(completion);
            }
            if !job_completions.is_empty() {
                self.process_job_completions(job_completions).await;
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
}
