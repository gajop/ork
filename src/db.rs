use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Run, Task, Workflow};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    // Workflow operations
    #[allow(clippy::too_many_arguments)]
    pub async fn create_workflow(
        &self,
        name: &str,
        description: Option<&str>,
        cloud_run_job_name: &str,
        cloud_run_region: &str,
        cloud_run_project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
    ) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            INSERT INTO workflows (name, description, cloud_run_job_name, cloud_run_region, cloud_run_project, executor_type, task_params)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(cloud_run_job_name)
        .bind(cloud_run_region)
        .bind(cloud_run_project)
        .bind(executor_type)
        .bind(task_params)
        .fetch_one(&self.pool)
        .await?;

        Ok(workflow)
    }

    pub async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            SELECT * FROM workflows WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await?;

        Ok(workflow)
    }

    pub async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        let workflows = sqlx::query_as::<_, Workflow>(
            r#"
            SELECT * FROM workflows ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(workflows)
    }

    // Run operations
    pub async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>(
            r#"
            INSERT INTO runs (workflow_id, status, triggered_by)
            VALUES ($1, 'pending', $2)
            RETURNING *
            "#,
        )
        .bind(workflow_id)
        .bind(triggered_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(run)
    }

    pub async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE runs
            SET status = $1,
                error = $2,
                started_at = COALESCE(started_at, CASE WHEN $1 = 'running' THEN NOW() ELSE NULL END),
                finished_at = CASE WHEN $1 IN ('success', 'failed', 'cancelled') THEN NOW() ELSE NULL END
            WHERE id = $3
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(run_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>(
            r#"
            SELECT * FROM runs WHERE id = $1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(run)
    }

    pub async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = if let Some(wf_id) = workflow_id {
            sqlx::query_as::<_, Run>(
                r#"
                SELECT * FROM runs WHERE workflow_id = $1 ORDER BY created_at DESC LIMIT 50
                "#,
            )
            .bind(wf_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Run>(
                r#"
                SELECT * FROM runs ORDER BY created_at DESC LIMIT 50
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(runs)
    }

    pub async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            r#"
            SELECT * FROM runs WHERE status = 'pending' ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(runs)
    }

    // Task operations
    pub async fn create_task(
        &self,
        run_id: Uuid,
        task_index: i32,
        params: Option<serde_json::Value>,
    ) -> Result<Task> {
        let task = sqlx::query_as::<_, Task>(
            r#"
            INSERT INTO tasks (run_id, task_index, status, params)
            VALUES ($1, $2, 'pending', $3)
            RETURNING *
            "#,
        )
        .bind(run_id)
        .bind(task_index)
        .bind(params)
        .fetch_one(&self.pool)
        .await?;

        Ok(task)
    }

    pub async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = $1,
                cloud_run_execution_name = COALESCE($2, cloud_run_execution_name),
                error = $3,
                dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = 'dispatched' THEN NOW() ELSE NULL END),
                started_at = COALESCE(started_at, CASE WHEN $1 = 'running' THEN NOW() ELSE NULL END),
                finished_at = CASE WHEN $1 IN ('success', 'failed', 'cancelled') THEN NOW() ELSE NULL END
            WHERE id = $4
            "#,
        )
        .bind(status)
        .bind(execution_name)
        .bind(error)
        .bind(task_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks WHERE run_id = $1 ORDER BY task_index ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    pub async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks WHERE status = 'pending' ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    pub async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT * FROM tasks WHERE status IN ('dispatched', 'running') ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
