use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;
use sqlx::Row;
use ork_core::database::NewTask;
use ork_core::models::{Task, TaskWithWorkflow};
use super::core::PostgresDatabase;

impl PostgresDatabase {
    pub(super) async fn batch_create_tasks_impl(&self, run_id: Uuid, task_count: i32, workflow_name: &str, executor_type: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for i in 0..task_count {
            let params = serde_json::json!({"task_index": i, "run_id": run_id, "workflow_name": workflow_name});
            sqlx::query("INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, params) VALUES ($1, $2, $3, $4, $5, 'pending', $6)")
                .bind(run_id).bind(i).bind(format!("task_{}", i)).bind(executor_type).bind(&Vec::<String>::new()).bind(params).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn batch_create_dag_tasks_impl(&self, run_id: Uuid, tasks: &[NewTask]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for task in tasks {
            sqlx::query("INSERT INTO tasks (run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params) VALUES ($1, $2, $3, $4, $5, 'pending', 0, $6, $7, $8)")
                .bind(run_id).bind(task.task_index).bind(&task.task_name).bind(&task.executor_type).bind(&task.depends_on).bind(task.max_retries).bind(task.timeout_seconds).bind(&task.params).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn update_task_status_impl(&self, task_id: Uuid, status: &str, execution_name: Option<&str>, error: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"UPDATE tasks SET status = $1, execution_name = COALESCE($2, execution_name), error = $3,
            attempts = CASE WHEN $1 = 'failed' THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN $1 = 'pending' THEN retry_at ELSE NULL END,
            dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = 'dispatched' THEN NOW() ELSE NULL END),
            started_at = COALESCE(started_at, CASE WHEN $1 = 'running' THEN NOW() ELSE NULL END),
            finished_at = CASE WHEN $1 IN ('success', 'failed', 'cancelled') THEN NOW() ELSE NULL END
            WHERE id = $4"#
        ).bind(status).bind(execution_name).bind(error).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn batch_update_task_status_impl(&self, updates: &[(Uuid, &str, Option<&str>, Option<&str>)]) -> Result<()> {
        if updates.is_empty() { return Ok(()); }
        let mut tx = self.pool.begin().await?;
        for (task_id, status, execution_name, error) in updates {
            sqlx::query(
                r#"UPDATE tasks SET status = $1, execution_name = COALESCE($2, execution_name), error = $3,
                attempts = CASE WHEN $1 = 'failed' THEN attempts + 1 ELSE attempts END,
                retry_at = CASE WHEN $1 = 'pending' THEN retry_at ELSE NULL END,
                dispatched_at = COALESCE(dispatched_at, CASE WHEN $1 = 'dispatched' THEN NOW() ELSE NULL END),
                started_at = COALESCE(started_at, CASE WHEN $1 = 'running' THEN NOW() ELSE NULL END),
                finished_at = CASE WHEN $1 IN ('success', 'failed', 'cancelled') THEN NOW() ELSE NULL END
                WHERE id = $4 AND (status NOT IN ('success', 'failed') OR $1 IN ('success', 'failed'))"#
            ).bind(status).bind(execution_name).bind(error).bind(task_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE run_id = $1 ORDER BY task_index").bind(run_id).fetch_all(&self.pool).await?;
        Ok(tasks)
    }
    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status = 'pending'").fetch_all(&self.pool).await?;
        Ok(tasks)
    }
    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE status IN ('running', 'dispatched')").fetch_all(&self.pool).await?;
        Ok(tasks)
    }
    pub(super) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query("UPDATE tasks SET logs = COALESCE(SUBSTRING(logs FROM 1 FOR 2000000), '') || $1 WHERE id = $2").bind(chunk).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn update_task_output_impl(&self, task_id: Uuid, output: serde_json::Value) -> Result<()> {
        sqlx::query("UPDATE tasks SET output = $1 WHERE id = $2").bind(output).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn reset_task_for_retry_impl(&self, task_id: Uuid, error: Option<&str>, retry_at: Option<chrono::DateTime<Utc>>) -> Result<()> {
        sqlx::query("UPDATE tasks SET status = 'pending', attempts = attempts + 1, retry_at = $1, error = COALESCE($2, error), execution_name = NULL, output = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL WHERE id = $3")
            .bind(retry_at).bind(error).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }
    pub(super) async fn get_pending_tasks_with_workflow_impl(&self, limit: i64) -> Result<Vec<TaskWithWorkflow>> {
        let tasks = sqlx::query_as::<_, TaskWithWorkflow>("SELECT t.id as task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on, t.status as task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params, w.id as workflow_id, w.job_name, w.project, w.region FROM tasks t JOIN runs r ON t.run_id = r.id JOIN workflows w ON r.workflow_id = w.id WHERE t.status = 'pending' AND (t.retry_at IS NULL OR t.retry_at <= NOW()) ORDER BY t.created_at LIMIT $1")
            .bind(limit).fetch_all(&self.pool).await?;
        Ok(tasks)
    }
    pub(super) async fn get_task_outputs_impl(&self, run_id: Uuid, task_names: &[String]) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() { return Ok(HashMap::new()); }
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
    pub(super) async fn get_task_retry_meta_impl(&self, task_ids: &[Uuid]) -> Result<HashMap<Uuid, (i32, i32)>> {
        if task_ids.is_empty() { return Ok(HashMap::new()); }
        let rows = sqlx::query("SELECT id, attempts, max_retries FROM tasks WHERE id = ANY($1)").bind(task_ids).fetch_all(&self.pool).await?;
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
        let row = sqlx::query("SELECT run_id FROM tasks WHERE id = $1").bind(task_id).fetch_one(&self.pool).await?;
        Ok(row.get("run_id"))
    }
    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let row = sqlx::query("SELECT run_id, task_name FROM tasks WHERE id = $1").bind(task_id).fetch_one(&self.pool).await?;
        Ok((row.get("run_id"), row.get("task_name")))
    }
    pub(super) async fn mark_tasks_failed_by_dependency_impl(&self, run_id: Uuid, failed_task_names: &[String], error: &str) -> Result<Vec<String>> {
        if failed_task_names.is_empty() { return Ok(Vec::new()); }
        let rows = sqlx::query("UPDATE tasks SET status = 'failed', error = $1, finished_at = NOW() WHERE run_id = $2 AND status = 'pending' AND depends_on && $3 RETURNING task_name")
            .bind(error).bind(run_id).bind(failed_task_names).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| row.get("task_name")).collect())
    }
}
