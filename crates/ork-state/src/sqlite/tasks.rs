use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use ork_core::database::NewTask;
use ork_core::models::{Task, TaskWithWorkflow};

use super::core::{SqliteDatabase, TaskRow, TaskWithWorkflowRow, encode_depends_on};

impl SqliteDatabase {
    pub(super) async fn batch_create_tasks_impl(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for i in 0..task_count {
            let task_id = Uuid::new_v4();
            let params_json = sqlx::types::Json(serde_json::json!({
                "task_index": i, "run_id": run_id, "workflow_name": workflow_name,
            }));
            sqlx::query(
                r#"INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, params)
                VALUES (?, ?, ?, ?, ?, '[]', 'pending', ?)"#,
            )
            .bind(task_id).bind(run_id).bind(i).bind(format!("task_{}", i)).bind(executor_type).bind(params_json)
            .execute(&mut *tx).await?;
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
            let task_id = Uuid::new_v4();
            let depends_on_json = encode_depends_on(&task.depends_on);
            let params_json = sqlx::types::Json(&task.params);
            sqlx::query(
                r#"INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, attempts, max_retries, timeout_seconds, params)
                VALUES (?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?, ?)"#,
            )
            .bind(task_id).bind(run_id).bind(task.task_index).bind(&task.task_name).bind(&task.executor_type)
            .bind(&depends_on_json).bind(task.max_retries).bind(task.timeout_seconds).bind(params_json)
            .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn update_task_status_impl(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE tasks SET status = ?, execution_name = COALESCE(?, execution_name), error = COALESCE(?, error),
            attempts = CASE WHEN ? = 'failed' THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN ? IN ('pending', 'paused') THEN retry_at ELSE NULL END,
            dispatched_at = CASE WHEN ? = 'dispatched' AND dispatched_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE dispatched_at END,
            started_at = CASE WHEN ? = 'running' AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
            finished_at = CASE WHEN ? IN ('success', 'failed') AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status).bind(execution_name).bind(error).bind(status).bind(status).bind(status).bind(status).bind(status).bind(task_id)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub(super) async fn batch_update_task_status_impl(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for (task_id, status, execution_name, error) in updates {
            self.update_task_status_impl_tx(&mut tx, *task_id, status, *execution_name, *error)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn update_task_status_impl_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE tasks SET status = ?, execution_name = COALESCE(?, execution_name), error = COALESCE(?, error),
            attempts = CASE WHEN ? = 'failed' THEN attempts + 1 ELSE attempts END,
            retry_at = CASE WHEN ? IN ('pending', 'paused') THEN retry_at ELSE NULL END,
            dispatched_at = CASE WHEN ? = 'dispatched' AND dispatched_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE dispatched_at END,
            started_at = CASE WHEN ? = 'running' AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
            finished_at = CASE WHEN ? IN ('success', 'failed') AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status).bind(execution_name).bind(error).bind(status).bind(status).bind(status).bind(status).bind(status).bind(task_id)
        .execute(&mut **tx).await?;
        Ok(())
    }

    pub(super) async fn list_tasks_impl(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE run_id = ? ORDER BY task_index",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn get_pending_tasks_impl(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE status = 'pending'")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn get_running_tasks_impl(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE status IN ('running', 'dispatched')",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    pub(super) async fn append_task_log_impl(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET logs = COALESCE(SUBSTR(logs, 1, 2000000), '') || ? WHERE id = ?",
        )
        .bind(chunk)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn update_task_output_impl(
        &self,
        task_id: Uuid,
        output: serde_json::Value,
    ) -> Result<()> {
        sqlx::query("UPDATE tasks SET output = ? WHERE id = ?")
            .bind(sqlx::types::Json(output))
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
        let retry_str = retry_at.map(|dt| dt.to_rfc3339_opts(SecondsFormat::Secs, true));
        sqlx::query(
            r#"UPDATE tasks SET status = 'pending', attempts = attempts + 1, retry_at = ?,
            error = COALESCE(?, error), execution_name = NULL, output = NULL, dispatched_at = NULL, started_at = NULL, finished_at = NULL
            WHERE id = ?"#,
        )
        .bind(retry_str).bind(error).bind(task_id).execute(&self.pool).await?;
        Ok(())
    }

    pub(super) async fn get_pending_tasks_with_workflow_impl(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let rows = sqlx::query_as::<_, TaskWithWorkflowRow>(
            r#"SELECT t.id as task_id, t.run_id, t.task_index, t.task_name, t.executor_type, t.depends_on,
            t.status as task_status, t.attempts, t.max_retries, t.timeout_seconds, t.retry_at, t.execution_name, t.params,
            w.id as workflow_id, w.job_name, w.project, w.region
            FROM tasks t JOIN runs r ON t.run_id = r.id JOIN workflows w ON r.workflow_id = w.id
            WHERE t.status = 'pending'
            AND r.status = 'running'
            AND (t.retry_at IS NULL OR datetime(t.retry_at) <= datetime('now'))
            AND (t.depends_on = '[]' OR NOT EXISTS (
                SELECT 1 FROM json_each(t.depends_on) AS dep
                WHERE NOT EXISTS (
                    SELECT 1 FROM tasks t2
                    WHERE t2.run_id = t.run_id
                    AND t2.task_name = dep.value
                    AND t2.status IN ('success', 'failed')
                )
            ))
            ORDER BY t.created_at LIMIT ?"#,
        )
        .bind(limit).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(Self::map_task_with_workflow).collect())
    }

    pub(super) async fn get_task_outputs_impl(
        &self,
        run_id: Uuid,
        task_names: &[String],
    ) -> Result<HashMap<String, serde_json::Value>> {
        if task_names.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = task_names.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT task_name, output FROM tasks WHERE run_id = ? AND task_name IN ({}) AND output IS NOT NULL",
            placeholders
        );
        let mut query = sqlx::query(&query_str).bind(run_id);
        for name in task_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map = HashMap::new();
        for row in rows {
            let name: String = row.get("task_name");
            let output: sqlx::types::Json<serde_json::Value> = row.get("output");
            map.insert(name, output.0);
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
        let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT id, attempts, max_retries FROM tasks WHERE id IN ({})",
            placeholders
        );
        let mut query = sqlx::query(&query_str);
        for id in task_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&self.pool).await?;
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
        let row = sqlx::query("SELECT run_id FROM tasks WHERE id = ?")
            .bind(task_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get("run_id"))
    }

    pub(super) async fn get_task_identity_impl(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let row = sqlx::query("SELECT run_id, task_name FROM tasks WHERE id = ?")
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
        let placeholders = failed_task_names
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let query_str = format!(
            r#"UPDATE tasks SET status = 'failed', error = ?, finished_at = CURRENT_TIMESTAMP
            WHERE run_id = ? AND status IN ('pending', 'paused') AND EXISTS (
                SELECT 1 FROM json_each(depends_on) WHERE value IN ({})
            ) RETURNING task_name"#,
            placeholders
        );
        let mut query = sqlx::query(&query_str).bind(error).bind(run_id);
        for name in failed_task_names {
            query = query.bind(name);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|row| row.get("task_name")).collect())
    }
}
