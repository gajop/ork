// FileDatabase - File-based implementation of the Database trait
//
// This stores workflows, runs, and tasks as JSON files in a directory structure:
// - workflows/{workflow_id}.json
// - runs/{run_id}.json
// - tasks/{task_id}.json

mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod workflows;

pub use core::FileDatabase;

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

use ork_core::database::{Database, NewTask, NewWorkflowTask};
use ork_core::models::{DeferredJob, Run, Task, TaskWithWorkflow, Workflow, WorkflowTask};

#[async_trait]
impl Database for FileDatabase {
    async fn run_migrations(&self) -> Result<()> {
        self.ensure_dirs().await?;
        Ok(())
    }

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
    ) -> Result<Workflow> {
        self.create_workflow_impl(
            name,
            description,
            job_name,
            region,
            project,
            executor_type,
            task_params,
            schedule,
        )
        .await
    }

    async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        self.get_workflow_impl(name).await
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        self.list_workflows_impl().await
    }

    async fn delete_workflow(&self, name: &str) -> Result<()> {
        self.delete_workflow_impl(name).await
    }

    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        self.create_run_impl(workflow_id, triggered_by).await
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_run_status_impl(run_id, status, error).await
    }

    async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        self.get_run_impl(run_id).await
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        self.list_runs_impl(workflow_id).await
    }

    async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        self.get_pending_runs_impl().await
    }

    async fn cancel_run(&self, run_id: Uuid) -> Result<()> {
        self.cancel_run_impl(run_id).await
    }

    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        self.batch_create_tasks_impl(run_id, task_count, workflow_name, executor_type)
            .await
    }

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> Result<()> {
        self.batch_create_dag_tasks_impl(run_id, tasks).await
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_task_status_impl(task_id, status, execution_name, error)
            .await
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        self.batch_update_task_status_impl(updates).await
    }

    async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        self.list_tasks_impl(run_id).await
    }

    async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        self.get_pending_tasks_impl().await
    }

    async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        self.get_running_tasks_impl().await
    }

    async fn append_task_log(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        self.append_task_log_impl(task_id, chunk).await
    }

    async fn update_task_output(&self, task_id: Uuid, output: serde_json::Value) -> Result<()> {
        self.update_task_output_impl(task_id, output).await
    }

    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        self.reset_task_for_retry_impl(task_id, error, retry_at)
            .await
    }

    async fn get_pending_tasks_with_workflow(&self, _limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        anyhow::bail!(
            "FileDatabase::get_pending_tasks_with_workflow not yet implemented - use get_pending_tasks instead"
        );
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        self.get_workflows_by_ids_impl(workflow_ids).await
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        self.get_workflow_by_id_impl(workflow_id).await
    }

    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        self.get_task_outputs_impl(run_id, task_names).await
    }

    async fn get_task_retry_meta(&self, task_ids: &[Uuid]) -> Result<HashMap<Uuid, (i32, i32)>> {
        self.get_task_retry_meta_impl(task_ids).await
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        self.get_task_run_id_impl(task_id).await
    }

    async fn get_task_identity(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        self.get_task_identity_impl(task_id).await
    }

    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        self.mark_tasks_failed_by_dependency_impl(run_id, failed_task_names, error)
            .await
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        self.get_run_task_stats_impl(run_id).await
    }

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        self.create_workflow_tasks_impl(workflow_id, tasks).await
    }

    async fn list_workflow_tasks(&self, workflow_id: Uuid) -> Result<Vec<WorkflowTask>> {
        self.list_workflow_tasks_impl(workflow_id).await
    }

    async fn get_due_scheduled_workflows(&self) -> Result<Vec<Workflow>> {
        self.get_due_scheduled_workflows_impl().await
    }

    async fn update_workflow_schedule_times(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: chrono::DateTime<chrono::Utc>,
        next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        self.update_workflow_schedule_times_impl(workflow_id, last_scheduled_at, next_scheduled_at)
            .await
    }

    async fn update_workflow_schedule(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> Result<()> {
        self.update_workflow_schedule_impl(workflow_id, schedule, schedule_enabled)
            .await
    }

    // Deferred job operations
    async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> Result<DeferredJob> {
        self.create_deferred_job(task_id, service_type, job_id, job_data)
            .await
    }

    async fn get_pending_deferred_jobs(&self) -> Result<Vec<DeferredJob>> {
        self.get_pending_deferred_jobs().await
    }

    async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<Vec<DeferredJob>> {
        self.get_deferred_jobs_for_task(task_id).await
    }

    async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        self.update_deferred_job_status(job_id, status, error).await
    }

    async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        self.update_deferred_job_polled(job_id).await
    }

    async fn complete_deferred_job(&self, job_id: Uuid) -> Result<()> {
        self.complete_deferred_job(job_id).await
    }

    async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> Result<()> {
        self.fail_deferred_job(job_id, error).await
    }

    async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<()> {
        self.cancel_deferred_jobs_for_task(task_id).await
    }
}
