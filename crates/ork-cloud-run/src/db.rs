use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Run, Task, TaskWithWorkflow, Workflow};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_pool_size(database_url, 10).await
    }

    pub async fn new_with_pool_size(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .idle_timeout(Some(std::time::Duration::from_secs(300)))
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
        job_name: &str,
        region: &str,
        project: &str,
        executor_type: &str,
        task_params: Option<serde_json::Value>,
    ) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            INSERT INTO workflows (name, description, job_name, region, project, executor_type, task_params)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(job_name)
        .bind(region)
        .bind(project)
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

    pub async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
    ) -> Result<()> {
        // Use a transaction for batch inserts
        let mut tx = self.pool.begin().await?;

        for i in 0..task_count {
            let params = serde_json::json!({
                "task_index": i,
                "run_id": run_id.to_string(),
                "workflow_name": workflow_name,
            });

            sqlx::query(
                r#"
                INSERT INTO tasks (run_id, task_index, status, params)
                VALUES ($1, $2, 'pending', $3)
                "#,
            )
            .bind(run_id)
            .bind(i)
            .bind(params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    #[allow(dead_code)]
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
                execution_name = COALESCE($2, execution_name),
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    // Optimized batch queries to avoid N+1 queries

    /// Get pending tasks with workflow info in a single query (eliminates N+1)
    pub async fn get_pending_tasks_with_workflow(&self, limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>(
            r#"
            SELECT
                t.id as task_id,
                t.run_id,
                t.task_index,
                t.status as task_status,
                t.execution_name,
                t.params,
                w.id as workflow_id,
                w.executor_type,
                w.job_name,
                w.project,
                w.region
            FROM tasks t
            INNER JOIN runs r ON t.run_id = r.id
            INNER JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = 'pending'
            ORDER BY t.created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    /// Get running/dispatched tasks with workflow info in a single query
    pub async fn get_running_tasks_with_workflow(&self, limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>(
            r#"
            SELECT
                t.id as task_id,
                t.run_id,
                t.task_index,
                t.status as task_status,
                t.execution_name,
                t.params,
                w.id as workflow_id,
                w.executor_type,
                w.job_name,
                w.project,
                w.region
            FROM tasks t
            INNER JOIN runs r ON t.run_id = r.id
            INNER JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status IN ('dispatched', 'running')
              AND t.execution_name IS NOT NULL
            ORDER BY t.created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    /// Batch update task statuses (more efficient than one-by-one)
    #[allow(dead_code)]
    pub async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        // Use transaction for atomicity
        let mut tx = self.pool.begin().await?;

        for (task_id, status, execution_name, error) in updates {
            sqlx::query(
                r#"
                UPDATE tasks
                SET status = $1,
                    execution_name = COALESCE($2, execution_name),
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
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
