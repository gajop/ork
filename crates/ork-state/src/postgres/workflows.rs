use anyhow::Result;
use uuid::Uuid;
use ork_core::database::NewWorkflowTask;
use ork_core::models::{Workflow, WorkflowTask};
use super::core::PostgresDatabase;

impl PostgresDatabase {
    pub(super) async fn create_workflow_impl(&self, name: &str, description: Option<&str>, job_name: &str, region: &str, project: &str, executor_type: &str, task_params: Option<serde_json::Value>, schedule: Option<&str>) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("INSERT INTO workflows (name, description, job_name, region, project, executor_type, task_params, schedule) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *")
            .bind(name).bind(description).bind(job_name).bind(region).bind(project).bind(executor_type).bind(task_params).bind(schedule).fetch_one(&self.pool).await?;
        Ok(workflow)
    }
    pub(super) async fn get_workflow_impl(&self, name: &str) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE name = $1").bind(name).fetch_one(&self.pool).await?;
        Ok(workflow)
    }
    pub(super) async fn list_workflows_impl(&self) -> Result<Vec<Workflow>> {
        let workflows = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows ORDER BY created_at DESC").fetch_all(&self.pool).await?;
        Ok(workflows)
    }
    pub(super) async fn delete_workflow_impl(&self, name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM runs WHERE workflow_id = (SELECT id FROM workflows WHERE name = $1)").bind(name).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM workflows WHERE name = $1").bind(name).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn get_workflows_by_ids_impl(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        if workflow_ids.is_empty() { return Ok(Vec::new()); }
        let workflows = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = ANY($1)").bind(workflow_ids).fetch_all(&self.pool).await?;
        Ok(workflows)
    }
    pub(super) async fn get_workflow_by_id_impl(&self, workflow_id: Uuid) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = $1").bind(workflow_id).fetch_one(&self.pool).await?;
        Ok(workflow)
    }
    pub(super) async fn create_workflow_tasks_impl(&self, workflow_id: Uuid, tasks: &[NewWorkflowTask]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM workflow_tasks WHERE workflow_id = $1").bind(workflow_id).execute(&mut *tx).await?;
        for task in tasks {
            sqlx::query("INSERT INTO workflow_tasks (workflow_id, task_index, task_name, executor_type, depends_on, params, signature) VALUES ($1, $2, $3, $4, $5, $6, $7)")
                .bind(workflow_id)
                .bind(task.task_index)
                .bind(&task.task_name)
                .bind(&task.executor_type)
                .bind(&task.depends_on)
                .bind(&task.params)
                .bind(&task.signature)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn list_workflow_tasks_impl(&self, workflow_id: Uuid) -> Result<Vec<WorkflowTask>> {
        let tasks = sqlx::query_as::<_, WorkflowTask>("SELECT * FROM workflow_tasks WHERE workflow_id = $1 ORDER BY task_index").bind(workflow_id).fetch_all(&self.pool).await?;
        Ok(tasks)
    }

    pub(super) async fn get_due_scheduled_workflows_impl(&self) -> Result<Vec<Workflow>> {
        let workflows = sqlx::query_as::<_, Workflow>(
            r#"SELECT * FROM workflows
               WHERE schedule_enabled = true
               AND schedule IS NOT NULL
               AND (next_scheduled_at IS NULL OR next_scheduled_at <= NOW())
               ORDER BY next_scheduled_at NULLS FIRST"#
        ).fetch_all(&self.pool).await?;
        Ok(workflows)
    }

    pub(super) async fn update_workflow_schedule_times_impl(
        &self,
        workflow_id: Uuid,
        last_scheduled_at: chrono::DateTime<chrono::Utc>,
        next_scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE workflows SET last_scheduled_at = $1, next_scheduled_at = $2 WHERE id = $3"
        ).bind(last_scheduled_at).bind(next_scheduled_at).bind(workflow_id).execute(&self.pool).await?;
        Ok(())
    }

    pub(super) async fn update_workflow_schedule_impl(
        &self,
        workflow_id: Uuid,
        schedule: Option<&str>,
        schedule_enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE workflows SET schedule = $1, schedule_enabled = $2 WHERE id = $3"
        ).bind(schedule).bind(schedule_enabled).bind(workflow_id).execute(&self.pool).await?;
        Ok(())
    }
}
