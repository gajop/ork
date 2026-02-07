// Database trait for orchestration state
// Implementations: PostgresDatabase, SqliteDatabase

use uuid::Uuid;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::models::{DeferredJob, Run, Task, TaskWithWorkflow, Workflow};

#[derive(Debug, Clone)]
pub struct NewTask {
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: serde_json::Value,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct NewWorkflowTask {
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: serde_json::Value,
    pub signature: Option<serde_json::Value>,
}

/// Database interface for orchestration state management
///
/// This trait abstracts over different database backends (Postgres, SQLite, etc.)
/// All operations are transactional where appropriate.

#[async_trait]
pub trait Database: Send + Sync {
    // Migration operations
    async fn run_migrations(&self) -> anyhow::Result<()>;

    // Workflow operations
    async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
        schedule: Option<&str>,
    ) -> anyhow::Result<Workflow>;

    async fn get_workflow(&self, name: &str) -> anyhow::Result<Workflow>;
    async fn list_workflows(&self) -> anyhow::Result<Vec<Workflow>>;
    async fn delete_workflow(&self, name: &str) -> anyhow::Result<()>;

    // Run operations
    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> anyhow::Result<Run>;

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    async fn get_run(&self, run_id: Uuid) -> anyhow::Result<Run>;
    async fn list_runs(&self, workflow_id: Option<Uuid>) -> anyhow::Result<Vec<Run>>;
    async fn get_pending_runs(&self) -> anyhow::Result<Vec<Run>>;

    /// Cancel a run and all its pending/dispatched/running tasks
    async fn cancel_run(&self, run_id: Uuid) -> anyhow::Result<()>;

    // Task operations
    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> anyhow::Result<()>;

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> anyhow::Result<()>;

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> anyhow::Result<()>;

    async fn list_workflow_tasks(
        &self,
        workflow_id: Uuid,
    ) -> anyhow::Result<Vec<crate::models::WorkflowTask>>;

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Batch update task statuses (optimized for bulk operations)
    ///
    /// Format: Vec<(task_id, status, execution_name, error)>
    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> anyhow::Result<()>;

    async fn list_tasks(&self, run_id: Uuid) -> anyhow::Result<Vec<Task>>;
    async fn get_pending_tasks(&self) -> anyhow::Result<Vec<Task>>;
    async fn get_running_tasks(&self) -> anyhow::Result<Vec<Task>>;
    async fn append_task_log(&self, task_id: Uuid, chunk: &str) -> anyhow::Result<()>;
    async fn update_task_output(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> anyhow::Result<()>;
    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()>;

    /// Get pending tasks with workflow info in a single query (avoids N+1)
    /// Used by scheduler for efficient task dispatching
    async fn get_pending_tasks_with_workflow(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<TaskWithWorkflow>>;

    /// Batch fetch workflows by IDs (used by scheduler)
    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> anyhow::Result<Vec<Workflow>>;

    /// Get workflow by ID (used by scheduler)
    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> anyhow::Result<Workflow>;

    /// Fetch outputs for a set of task names in a run.
    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> anyhow::Result<HashMap<String, serde_json::Value>>;

    /// Fetch retry metadata for tasks.
    async fn get_task_retry_meta(
        &self,
        task_ids: &[Uuid],
    ) -> anyhow::Result<HashMap<Uuid, (i32, i32)>>;

    /// Get run ID for a task (used by scheduler to check run completion)
    async fn get_task_run_id(&self, task_id: Uuid) -> anyhow::Result<Uuid>;

    /// Get run ID + task name for a task (used for dependency propagation)
    async fn get_task_identity(&self, task_id: Uuid) -> anyhow::Result<(Uuid, String)>;

    /// Mark pending tasks as failed if they depend on any of the given task names.
    /// Returns the names of tasks updated (used for transitive failure propagation).
    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> anyhow::Result<Vec<String>>;

    /// Get run completion stats (total, completed, failed counts)
    /// Used by scheduler to check if a run is complete
    async fn get_run_task_stats(&self, run_id: Uuid) -> anyhow::Result<(i64, i64, i64)>;

    // Schedule operations
    /// Get workflows that have schedules enabled and are due for triggering
    async fn get_due_scheduled_workflows(&self) -> anyhow::Result<Vec<Workflow>>;

    /// Update schedule trigger times for a workflow
    async fn update_workflow_schedule_times(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: chrono::DateTime<chrono::Utc>,
        next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<()>;

    /// Update workflow schedule settings
    async fn update_workflow_schedule(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> anyhow::Result<()>;

    // Deferred job operations (for Triggerer component)

    /// Create a deferred job for tracking
    async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> anyhow::Result<DeferredJob>;

    /// Get all deferred jobs that need polling (pending or polling status)
    async fn get_pending_deferred_jobs(&self) -> anyhow::Result<Vec<DeferredJob>>;

    /// Get deferred jobs for a specific task
    async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<Vec<DeferredJob>>;

    /// Update deferred job status
    async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Update deferred job last polled timestamp
    async fn update_deferred_job_polled(&self, job_id: Uuid) -> anyhow::Result<()>;

    /// Mark deferred job as completed
    async fn complete_deferred_job(&self, job_id: Uuid) -> anyhow::Result<()>;

    /// Mark deferred job as failed
    async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> anyhow::Result<()>;

    /// Cancel all deferred jobs for a task
    async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> anyhow::Result<()>;
}
