macro_rules! impl_database_delegates {
    ($db_ty:ty, $run_migrations:path) => {
        #[async_trait::async_trait]
        impl ork_core::database::WorkflowRepository for $db_ty {
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
            ) -> anyhow::Result<ork_core::models::Workflow> {
                <$db_ty>::create_workflow_impl(
                    self,
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

            async fn get_workflow(&self, name: &str) -> anyhow::Result<ork_core::models::Workflow> {
                <$db_ty>::get_workflow_impl(self, name).await
            }

            async fn get_workflow_by_id(
                &self,
                workflow_id: uuid::Uuid,
            ) -> anyhow::Result<ork_core::models::Workflow> {
                <$db_ty>::get_workflow_by_id_impl(self, workflow_id).await
            }

            async fn list_workflows(&self) -> anyhow::Result<Vec<ork_core::models::Workflow>> {
                <$db_ty>::list_workflows_impl(self).await
            }

            async fn list_workflows_page(
                &self,
                query: &ork_core::database::WorkflowListQuery,
            ) -> anyhow::Result<ork_core::database::WorkflowListPage> {
                <$db_ty>::list_workflows_page_impl(self, query).await
            }

            async fn delete_workflow(&self, name: &str) -> anyhow::Result<()> {
                <$db_ty>::delete_workflow_impl(self, name).await
            }

            async fn get_workflows_by_ids(
                &self,
                workflow_ids: &[uuid::Uuid],
            ) -> anyhow::Result<Vec<ork_core::models::Workflow>> {
                <$db_ty>::get_workflows_by_ids_impl(self, workflow_ids).await
            }

            async fn create_workflow_tasks(
                &self,
                workflow_id: uuid::Uuid,
                tasks: &[ork_core::database::NewWorkflowTask],
            ) -> anyhow::Result<()> {
                <$db_ty>::create_workflow_tasks_impl(self, workflow_id, tasks).await
            }

            async fn list_workflow_tasks(
                &self,
                workflow_id: uuid::Uuid,
            ) -> anyhow::Result<Vec<ork_core::models::WorkflowTask>> {
                <$db_ty>::list_workflow_tasks_impl(self, workflow_id).await
            }
        }

        #[async_trait::async_trait]
        impl ork_core::database::RunRepository for $db_ty {
            async fn create_run(
                &self,
                workflow_id: uuid::Uuid,
                triggered_by: &str,
            ) -> anyhow::Result<ork_core::models::Run> {
                <$db_ty>::create_run_impl(self, workflow_id, triggered_by).await
            }

            async fn update_run_status(
                &self,
                run_id: uuid::Uuid,
                status: ork_core::models::RunStatus,
                error: Option<&str>,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_run_status_impl(self, run_id, status, error).await
            }

            async fn get_run(&self, run_id: uuid::Uuid) -> anyhow::Result<ork_core::models::Run> {
                <$db_ty>::get_run_impl(self, run_id).await
            }

            async fn list_runs(
                &self,
                workflow_id: Option<uuid::Uuid>,
            ) -> anyhow::Result<Vec<ork_core::models::Run>> {
                <$db_ty>::list_runs_impl(self, workflow_id).await
            }

            async fn list_runs_page(
                &self,
                query: &ork_core::database::RunListQuery,
            ) -> anyhow::Result<ork_core::database::RunListPage> {
                <$db_ty>::list_runs_page_impl(self, query).await
            }

            async fn get_pending_runs(&self) -> anyhow::Result<Vec<ork_core::models::Run>> {
                <$db_ty>::get_pending_runs_impl(self).await
            }

            async fn cancel_run(&self, run_id: uuid::Uuid) -> anyhow::Result<()> {
                <$db_ty>::cancel_run_impl(self, run_id).await
            }

            async fn get_run_task_stats(
                &self,
                run_id: uuid::Uuid,
            ) -> anyhow::Result<(i64, i64, i64)> {
                <$db_ty>::get_run_task_stats_impl(self, run_id).await
            }
        }

        #[async_trait::async_trait]
        impl ork_core::database::TaskRepository for $db_ty {
            async fn batch_create_tasks(
                &self,
                run_id: uuid::Uuid,
                task_count: i32,
                workflow_name: &str,
                executor_type: &str,
            ) -> anyhow::Result<()> {
                <$db_ty>::batch_create_tasks_impl(
                    self,
                    run_id,
                    task_count,
                    workflow_name,
                    executor_type,
                )
                .await
            }

            async fn batch_create_dag_tasks(
                &self,
                run_id: uuid::Uuid,
                tasks: &[ork_core::database::NewTask],
            ) -> anyhow::Result<()> {
                <$db_ty>::batch_create_dag_tasks_impl(self, run_id, tasks).await
            }

            async fn update_task_status(
                &self,
                task_id: uuid::Uuid,
                status: ork_core::models::TaskStatus,
                execution_name: Option<&str>,
                error: Option<&str>,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_task_status_impl(self, task_id, status, execution_name, error)
                    .await
            }

            async fn batch_update_task_status(
                &self,
                updates: &[(
                    uuid::Uuid,
                    ork_core::models::TaskStatus,
                    Option<&str>,
                    Option<&str>,
                )],
            ) -> anyhow::Result<()> {
                <$db_ty>::batch_update_task_status_impl(self, updates).await
            }

            async fn list_tasks(
                &self,
                run_id: uuid::Uuid,
            ) -> anyhow::Result<Vec<ork_core::models::Task>> {
                <$db_ty>::list_tasks_impl(self, run_id).await
            }

            async fn get_pending_tasks(&self) -> anyhow::Result<Vec<ork_core::models::Task>> {
                <$db_ty>::get_pending_tasks_impl(self).await
            }

            async fn get_running_tasks(&self) -> anyhow::Result<Vec<ork_core::models::Task>> {
                <$db_ty>::get_running_tasks_impl(self).await
            }

            async fn append_task_log(
                &self,
                task_id: uuid::Uuid,
                chunk: &str,
            ) -> anyhow::Result<()> {
                <$db_ty>::append_task_log_impl(self, task_id, chunk).await
            }

            async fn update_task_output(
                &self,
                task_id: uuid::Uuid,
                output: serde_json::Value,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_task_output_impl(self, task_id, output).await
            }

            async fn reset_task_for_retry(
                &self,
                task_id: uuid::Uuid,
                error: Option<&str>,
                retry_at: Option<chrono::DateTime<chrono::Utc>>,
            ) -> anyhow::Result<()> {
                <$db_ty>::reset_task_for_retry_impl(self, task_id, error, retry_at).await
            }

            async fn get_pending_tasks_with_workflow(
                &self,
                limit: i64,
            ) -> anyhow::Result<Vec<ork_core::models::TaskWithWorkflow>> {
                <$db_ty>::get_pending_tasks_with_workflow_impl(self, limit).await
            }

            async fn get_task_outputs(
                &self,
                run_id: uuid::Uuid,
                task_names: &[String],
            ) -> anyhow::Result<std::collections::HashMap<String, serde_json::Value>> {
                <$db_ty>::get_task_outputs_impl(self, run_id, task_names).await
            }

            async fn get_task_retry_meta(
                &self,
                task_ids: &[uuid::Uuid],
            ) -> anyhow::Result<std::collections::HashMap<uuid::Uuid, (i32, i32)>> {
                <$db_ty>::get_task_retry_meta_impl(self, task_ids).await
            }

            async fn get_task_run_id(&self, task_id: uuid::Uuid) -> anyhow::Result<uuid::Uuid> {
                <$db_ty>::get_task_run_id_impl(self, task_id).await
            }

            async fn get_task_identity(
                &self,
                task_id: uuid::Uuid,
            ) -> anyhow::Result<(uuid::Uuid, String)> {
                <$db_ty>::get_task_identity_impl(self, task_id).await
            }

            async fn mark_tasks_failed_by_dependency(
                &self,
                run_id: uuid::Uuid,
                failed_task_names: &[String],
                error: &str,
            ) -> anyhow::Result<Vec<String>> {
                <$db_ty>::mark_tasks_failed_by_dependency_impl(
                    self,
                    run_id,
                    failed_task_names,
                    error,
                )
                .await
            }
        }

        #[async_trait::async_trait]
        impl ork_core::database::ScheduleRepository for $db_ty {
            async fn get_due_scheduled_workflows(
                &self,
            ) -> anyhow::Result<Vec<ork_core::models::Workflow>> {
                <$db_ty>::get_due_scheduled_workflows_impl(self).await
            }

            async fn update_workflow_schedule_times(
                &self,
                workflow_id: uuid::Uuid,
                last_scheduled_at: chrono::DateTime<chrono::Utc>,
                next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_workflow_schedule_times_impl(
                    self,
                    workflow_id,
                    last_scheduled_at,
                    next_scheduled_at,
                )
                .await
            }

            async fn update_workflow_schedule(
                &self,
                workflow_id: uuid::Uuid,
                schedule: Option<&str>,
                schedule_enabled: bool,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_workflow_schedule_impl(
                    self,
                    workflow_id,
                    schedule,
                    schedule_enabled,
                )
                .await
            }
        }

        #[async_trait::async_trait]
        impl ork_core::database::DeferredJobRepository for $db_ty {
            async fn create_deferred_job(
                &self,
                task_id: uuid::Uuid,
                service_type: &str,
                job_id: &str,
                job_data: serde_json::Value,
            ) -> anyhow::Result<ork_core::models::DeferredJob> {
                <$db_ty>::create_deferred_job(self, task_id, service_type, job_id, job_data).await
            }

            async fn get_pending_deferred_jobs(
                &self,
            ) -> anyhow::Result<Vec<ork_core::models::DeferredJob>> {
                <$db_ty>::get_pending_deferred_jobs(self).await
            }

            async fn get_deferred_jobs_for_task(
                &self,
                task_id: uuid::Uuid,
            ) -> anyhow::Result<Vec<ork_core::models::DeferredJob>> {
                <$db_ty>::get_deferred_jobs_for_task(self, task_id).await
            }

            async fn update_deferred_job_status(
                &self,
                job_id: uuid::Uuid,
                status: ork_core::models::DeferredJobStatus,
                error: Option<&str>,
            ) -> anyhow::Result<()> {
                <$db_ty>::update_deferred_job_status(self, job_id, status, error).await
            }

            async fn update_deferred_job_polled(&self, job_id: uuid::Uuid) -> anyhow::Result<()> {
                <$db_ty>::update_deferred_job_polled(self, job_id).await
            }

            async fn complete_deferred_job(&self, job_id: uuid::Uuid) -> anyhow::Result<()> {
                <$db_ty>::complete_deferred_job(self, job_id).await
            }

            async fn fail_deferred_job(
                &self,
                job_id: uuid::Uuid,
                error: &str,
            ) -> anyhow::Result<()> {
                <$db_ty>::fail_deferred_job(self, job_id, error).await
            }

            async fn cancel_deferred_jobs_for_task(
                &self,
                task_id: uuid::Uuid,
            ) -> anyhow::Result<()> {
                <$db_ty>::cancel_deferred_jobs_for_task(self, task_id).await
            }
        }

        #[async_trait::async_trait]
        impl ork_core::database::Database for $db_ty {
            async fn run_migrations(&self) -> anyhow::Result<()> {
                $run_migrations(self).await
            }
        }
    };
}

pub(crate) use impl_database_delegates;
