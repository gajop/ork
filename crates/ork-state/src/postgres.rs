use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use std::collections::HashMap;

use ork_core::database::Database as DatabaseTrait;
use ork_core::models::{Run, Task, TaskWithWorkflow, Workflow};

pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
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

    pub async fn delete_workflow(&self, name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            DELETE FROM runs
            WHERE workflow_id = (SELECT id FROM workflows WHERE name = $1)
            "#,
        )
        .bind(name)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            DELETE FROM workflows WHERE name = $1
            "#,
        )
        .bind(name)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
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

    pub async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
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
                INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, params, attempts, max_retries, timeout_seconds, retry_at)
                VALUES ($1, $2, $3, $4, $5, 'pending', $6, 0, 0, NULL, NULL)
                "#,
            )
            .bind(run_id)
            .bind(i)
            .bind(format!("task_{}", i))
            .bind(executor_type)
            .bind(Vec::<String>::new())
            .bind(params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn batch_create_dag_tasks(
        &self,
        run_id: Uuid,
        tasks: &[ork_core::database::NewTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for task in tasks {
            sqlx::query(
                r#"
                INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, params, attempts, max_retries, timeout_seconds, retry_at)
                VALUES ($1, $2, $3, $4, $5, 'pending', $6, 0, $7, $8, NULL)
                "#,
            )
            .bind(run_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(&task.depends_on)
            .bind(&task.params)
            .bind(task.max_retries)
            .bind(task.timeout_seconds)
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
                attempts = CASE WHEN $1 = 'failed' THEN attempts + 1 ELSE attempts END,
                retry_at = CASE WHEN $1 = 'pending' THEN retry_at ELSE NULL END,
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
    pub async fn get_pending_tasks_with_workflow(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>(
            r#"
            SELECT
                t.id as task_id,
                t.run_id,
                t.task_index,
                t.task_name,
                t.executor_type,
                t.depends_on,
                t.status as task_status,
                t.attempts,
                t.max_retries,
                t.timeout_seconds,
                t.retry_at,
                t.execution_name,
                t.params,
                w.id as workflow_id,
                w.job_name,
                w.project,
                w.region
            FROM tasks t
            INNER JOIN runs r ON t.run_id = r.id
            INNER JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = 'pending'
              AND (t.retry_at IS NULL OR t.retry_at <= NOW())
              AND (
                array_length(t.depends_on, 1) IS NULL
                OR NOT EXISTS (
                    SELECT 1
                    FROM tasks dep
                    WHERE dep.run_id = t.run_id
                      AND dep.task_name = ANY (t.depends_on)
                      AND dep.status <> 'success'
                )
              )
            ORDER BY t.created_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    pub async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[ork_core::database::NewWorkflowTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            DELETE FROM workflow_tasks WHERE workflow_id = $1
            "#,
        )
        .bind(workflow_id)
        .execute(&mut *tx)
        .await?;

        for task in tasks {
            sqlx::query(
                r#"
                INSERT INTO workflow_tasks (workflow_id, task_index, task_name, executor_type, depends_on, params)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
            )
            .bind(workflow_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(&task.depends_on)
            .bind(&task.params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_workflow_tasks(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<ork_core::models::WorkflowTask>> {
        let tasks = sqlx::query_as::<_, ork_core::models::WorkflowTask>(
            r#"
            SELECT * FROM workflow_tasks
            WHERE workflow_id = $1
            ORDER BY task_index ASC
            "#,
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(tasks)
    }

    /// Batch update task statuses (more efficient than one-by-one)
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
                    attempts = CASE WHEN $1 = 'failed' THEN attempts + 1 ELSE attempts END,
                    retry_at = CASE WHEN $1 = 'pending' THEN retry_at ELSE NULL END,
                    dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = 'dispatched' THEN NOW() ELSE NULL END),
                    started_at = COALESCE(started_at, CASE WHEN $1 = 'running' THEN NOW() ELSE NULL END),
                    finished_at = CASE WHEN $1 IN ('success', 'failed', 'cancelled') THEN NOW() ELSE NULL END
                WHERE id = $4
                  -- Prevent race condition: don't overwrite terminal states (success/failed) with non-terminal states
                  -- This guards against late "dispatched" updates overwriting "success" from event-driven notifications
                  AND (status NOT IN ('success', 'failed') OR $1 IN ('success', 'failed'))
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

    // Scheduler-specific methods
    pub async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        let workflows = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = ANY($1)")
            .bind(workflow_ids)
            .fetch_all(&self.pool)
            .await?;
        Ok(workflows)
    }

    pub async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = $1")
            .bind(workflow_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(workflow)
    }

    pub async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        let (run_id,): (Uuid,) = sqlx::query_as("SELECT run_id FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(run_id)
    }

    pub async fn get_task_identity(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let (run_id, task_name): (Uuid, String) =
            sqlx::query_as("SELECT run_id, task_name FROM tasks WHERE id = $1")
                .bind(task_id)
                .fetch_one(&self.pool)
                .await?;
        Ok((run_id, task_name))
    }

    pub async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, (String,)>(
            r#"
            UPDATE tasks
            SET status = 'failed',
                error = $3,
                finished_at = NOW()
            WHERE run_id = $1
              AND status = 'pending'
              AND depends_on && $2::text[]
            RETURNING task_name
            "#,
        )
        .bind(run_id)
        .bind(failed_task_names)
        .bind(error)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|(name,)| name).collect())
    }

    pub async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let stats: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status IN ('success', 'failed')) as completed,
                COUNT(*) FILTER (WHERE status = 'failed') as failed
            FROM tasks
            WHERE run_id = $1
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(stats)
    }
}

// Implement the Database trait from ork-core
#[async_trait]
impl DatabaseTrait for PostgresDatabase {
    async fn run_migrations(&self) -> Result<()> {
        PostgresDatabase::run_migrations(self).await
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
    ) -> Result<Workflow> {
        PostgresDatabase::create_workflow(
            self,
            name,
            description,
            job_name,
            region,
            project,
            executor_type,
            task_params,
        )
        .await
    }

    async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        PostgresDatabase::get_workflow(self, name).await
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        PostgresDatabase::list_workflows(self).await
    }

    async fn delete_workflow(&self, name: &str) -> Result<()> {
        PostgresDatabase::delete_workflow(self, name).await
    }

    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        PostgresDatabase::create_run(self, workflow_id, triggered_by).await
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        PostgresDatabase::update_run_status(self, run_id, status, error).await
    }

    async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        PostgresDatabase::get_run(self, run_id).await
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        PostgresDatabase::list_runs(self, workflow_id).await
    }

    async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        PostgresDatabase::get_pending_runs(self).await
    }

    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        PostgresDatabase::batch_create_tasks(self, run_id, task_count, workflow_name, executor_type)
            .await
    }

    async fn batch_create_dag_tasks(
        &self,
        run_id: Uuid,
        tasks: &[ork_core::database::NewTask],
    ) -> Result<()> {
        PostgresDatabase::batch_create_dag_tasks(self, run_id, tasks).await
    }

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[ork_core::database::NewWorkflowTask],
    ) -> Result<()> {
        PostgresDatabase::create_workflow_tasks(self, workflow_id, tasks).await
    }

    async fn list_workflow_tasks(
        &self,
        workflow_id: Uuid,
    ) -> Result<Vec<ork_core::models::WorkflowTask>> {
        PostgresDatabase::list_workflow_tasks(self, workflow_id).await
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        PostgresDatabase::update_task_status(self, task_id, status, execution_name, error).await
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        PostgresDatabase::batch_update_task_status(self, updates).await
    }

    async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        PostgresDatabase::list_tasks(self, run_id).await
    }

    async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        PostgresDatabase::get_pending_tasks(self).await
    }

    async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        PostgresDatabase::get_running_tasks(self).await
    }

    async fn append_task_log(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET logs = COALESCE(logs, '') || $2
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .bind(chunk)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_task_output(&self, task_id: Uuid, output: serde_json::Value) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET output = $2
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .bind(output)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn reset_task_for_retry(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'pending',
                execution_name = NULL,
                error = $2,
                attempts = attempts + 1,
                retry_at = $3,
                dispatched_at = NULL,
                started_at = NULL,
                finished_at = NULL,
                output = NULL
            WHERE id = $1
            "#,
        )
        .bind(task_id)
        .bind(error)
        .bind(retry_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_pending_tasks_with_workflow(&self, limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        PostgresDatabase::get_pending_tasks_with_workflow(self, limit).await
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        PostgresDatabase::get_workflows_by_ids(self, workflow_ids).await
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        PostgresDatabase::get_workflow_by_id(self, workflow_id).await
    }

    async fn get_task_outputs(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query_as::<_, (String, Option<serde_json::Value>)>(
            r#"
            SELECT task_name, output
            FROM tasks
            WHERE run_id = $1
              AND task_name = ANY($2)
            "#,
        )
        .bind(run_id)
        .bind(task_names)
        .fetch_all(&self.pool)
        .await?;

        let mut map = HashMap::new();
        for (name, output) in rows {
            if let Some(value) = output {
                map.insert(name, value);
            }
        }
        Ok(map)
    }

    async fn get_task_retry_meta(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, (i32, i32)>> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query_as::<_, (Uuid, i32, i32)>(
            r#"
            SELECT id, attempts, max_retries
            FROM tasks
            WHERE id = ANY($1)
            "#,
        )
        .bind(task_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut map = HashMap::new();
        for (id, attempts, max_retries) in rows {
            map.insert(id, (attempts, max_retries));
        }
        Ok(map)
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        PostgresDatabase::get_task_run_id(self, task_id).await
    }

    async fn get_task_identity(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        PostgresDatabase::get_task_identity(self, task_id).await
    }

    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        PostgresDatabase::mark_tasks_failed_by_dependency(self, run_id, failed_task_names, error)
            .await
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        PostgresDatabase::get_run_task_stats(self, run_id).await
    }
}
