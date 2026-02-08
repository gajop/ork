use super::core::PostgresDatabase;
use anyhow::Result;
use chrono::Utc;
use ork_core::database::NewTask;
use ork_core::models::{RunStatus, Task, TaskStatus, TaskWithWorkflow};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

impl PostgresDatabase {
    pub(super) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for i in 0..task_count {
            let params = serde_json::json!({"task_index": i, "run_id": run_id, "workflow_name": workflow_name});
            sqlx::query("INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, params) VALUES ($1, $2, $3, $4, $5, $6, $7)")
                .bind(run_id)
                .bind(i)
                .bind(format!("task_{}", i))
                .bind(executor_type)
                .bind(Vec::<String>::new())
                .bind(TaskStatus::Pending.as_str())
                .bind(params)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn batch_create_dag_tasks_impl(
        &self,
        run_id: Uuid,
        tasks: &[NewTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for task in tasks {
            sqlx::query("INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params) VALUES ($1, $2, $3, $4, $5, $6, 0, $7, $8, $9)")
                .bind(run_id)
                .bind(task.task_index)
                .bind(&task.task_name)
                .bind(&task.executor_type)
                .bind(&task.depends_on)
                .bind(TaskStatus::Pending.as_str())
                .bind(task.max_retries)
                .bind(task.timeout_seconds)
                .bind(&task.params)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn update_task_status_impl(
        &self,
        task_id: Uuid,
        status: TaskStatus,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"UPDATE tasks SET status = $1, execution_name = COALESCE($2, execution_name), error = $3,
            attempts = CASE WHEN $1 = $5 THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN $1 IN ($6, $7) THEN retry_at ELSE NULL END,
            dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = $8 THEN NOW() ELSE NULL END),
            started_at = COALESCE(started_at, CASE WHEN $1 = $9 THEN NOW() ELSE NULL END),
            finished_at = CASE WHEN $1 IN ($10, $5, $11) THEN NOW() ELSE NULL END
            WHERE id = $4"#
        )
        .bind(status_str)
        .bind(execution_name)
        .bind(error)
        .bind(task_id)
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Pending.as_str())
        .bind(TaskStatus::Paused.as_str())
        .bind(TaskStatus::Dispatched.as_str())
        .bind(TaskStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Cancelled.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
    pub(super) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, TaskStatus, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for (task_id, status, execution_name, error) in updates {
            let status_str = status.as_str();
            sqlx::query(
                r#"UPDATE tasks SET status = $1, execution_name = COALESCE($2, execution_name), error = $3,
                attempts = CASE WHEN $1 = $5 THEN attempts + 1 ELSE attempts END,
                retry_at = CASE WHEN $1 IN ($6, $7) THEN retry_at ELSE NULL END,
                dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = $8 THEN NOW() ELSE NULL END),
                started_at = COALESCE(started_at, CASE WHEN $1 = $9 THEN NOW() ELSE NULL END),
                finished_at = CASE WHEN $1 IN ($10, $5, $11) THEN NOW() ELSE NULL END
                WHERE id = $4 AND (status NOT IN ($10, $5) OR $1 IN ($10, $5))"#
            )
            .bind(status_str)
            .bind(execution_name)
            .bind(error)
            .bind(task_id)
            .bind(TaskStatus::Failed.as_str())
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Paused.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .bind(TaskStatus::Running.as_str())
            .bind(TaskStatus::Success.as_str())
            .bind(TaskStatus::Cancelled.as_str())
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let tasks =
            sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE run_id = $1 ORDER BY task_index")
                .bind(run_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(tasks)
    }
    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status = $1")
            .bind(TaskStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }
    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status IN ($1, $2)")
            .bind(TaskStatus::Running.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(tasks)
    }
    pub(super) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query("UPDATE tasks SET logs = COALESCE(SUBSTRING(logs FROM 1 FOR 2000000), '') || $1 WHERE id = $2").bind(chunk).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn update_task_output_impl(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET output = $1 WHERE id = $2")
            .bind(output)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn reset_task_for_retry_impl(
        &self,
        task_id: Uuid,
        error: Option<&str>,
        retry_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET status = $1, attempts = attempts + 1, retry_at = $2, error = COALESCE($3, error), execution_name = NULL, output = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL WHERE id = $4")
            .bind(TaskStatus::Pending.as_str())
            .bind(retry_at)
            .bind(error)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn get_pending_tasks_with_workflow_impl(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>(
            r#"SELECT t.id as task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on,
            t.status as task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params,
            w.id as workflow_id, w.job_name, w.project, w.region
            FROM tasks t JOIN runs r ON t.run_id = r.id JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = $2
            AND r.status = $3
            AND (t.retry_at IS NULL OR t.retry_at <= NOW())
            AND (cardinality(t.depends_on) = 0 OR NOT EXISTS (
                SELECT 1 FROM unnest(t.depends_on) AS dep_name
                WHERE NOT EXISTS (
                    SELECT 1 FROM tasks t2
                    WHERE t2.run_id = t.run_id
                    AND t2.task_name = dep_name
                    AND t2.status IN ($4, $5)
                )
            ))
            ORDER BY t.created_at LIMIT $1"#
        )
        .bind(limit)
        .bind(TaskStatus::Pending.as_str())
        .bind(RunStatus::Running.as_str())
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(tasks)
    }
    pub(super) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query("SELECT task_name, output FROM tasks WHERE run_id = $1 AND task_name = ANY($2) AND output IS NOT NULL")
            .bind(run_id).bind(task_names).fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for row in rows {
            let name: String = row.get("task_name");
            let output: serde_json::Value = row.get("output");
            map.insert(name, output);
        }
        Ok(map)
    }
    pub(super) async fn get_task_retry_meta_impl(
        &self,
        task_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, (i32, i32)>> {
        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query("SELECT id, attempts, max_retries FROM tasks WHERE id = ANY($1)")
            .bind(task_ids)
            .fetch_all(&self.pool)
            .await?;
        let mut map = HashMap::new();
        for row in rows {
            let id: Uuid = row.get("id");
            let attempts: i32 = row.get("attempts");
            let max_retries: i32 = row.get("max_retries");
            map.insert(id, (attempts, max_retries));
        }
        Ok(map)
    }
    pub(super) async fn get_task_run_id_impl(&self, task_id: Uuid) -> Result<Uuid> {
        let row = sqlx::query("SELECT run_id FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("run_id"))
    }
    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let row = sqlx::query("SELECT run_id, task_name FROM tasks WHERE id = $1")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.get("run_id"), row.get("task_name")))
    }
    pub(super) async fn mark_tasks_failed_by_dependency_impl(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query("UPDATE tasks SET status = $1, error = $2, finished_at = NOW() WHERE run_id = $3 AND status IN ($4, $5) AND depends_on && $6 RETURNING task_name")
            .bind(TaskStatus::Failed.as_str())
            .bind(error)
            .bind(run_id)
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Paused.as_str())
            .bind(failed_task_names)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| row.get("task_name")).collect())
    }
}
