use anyhow::Result;
use ork_core::models::{Run, RunStatus, TaskStatus};
use sqlx::Row;
use uuid::Uuid;

use super::core::SqliteDatabase;

impl SqliteDatabase {
    pub(super) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        let run_id = Uuid::new_v4();
        let run = sqlx::query_as::<_, Run>(
            r#"INSERT INTO runs (id, workflow_id, status, triggered_by) VALUES (?, ?, ?, ?) RETURNING *"#,
        )
        .bind(run_id)
        .bind(workflow_id)
        .bind(RunStatus::Pending.as_str())
        .bind(triggered_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(run)
    }

    pub(super) async fn update_run_status_impl(
        &self,
        run_id: Uuid,
        status: RunStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let status_str = status.as_str();
        sqlx::query(
            r#"UPDATE runs SET status = ?, error = ?,
            started_at = CASE WHEN ? = ? AND started_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
            finished_at = CASE WHEN ? IN (?, ?, ?) AND finished_at IS NULL THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status_str)
        .bind(error)
        .bind(status_str)
        .bind(RunStatus::Running.as_str())
        .bind(status_str)
        .bind(RunStatus::Success.as_str())
        .bind(RunStatus::Failed.as_str())
        .bind(RunStatus::Cancelled.as_str())
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(super) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(run)
    }

    pub(super) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = match workflow_id {
            Some(wf_id) => {
                sqlx::query_as::<_, Run>(
                    "SELECT * FROM runs WHERE workflow_id = ? ORDER BY created_at DESC",
                )
                .bind(wf_id)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC")
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        Ok(runs)
    }

    pub(super) async fn get_pending_runs_impl(&self) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE status = ?")
            .bind(RunStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(runs)
    }

    pub(super) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE runs SET status = ?, finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND status NOT IN (?, ?, ?)")
            .bind(RunStatus::Cancelled.as_str())
            .bind(run_id)
            .bind(RunStatus::Success.as_str())
            .bind(RunStatus::Failed.as_str())
            .bind(RunStatus::Cancelled.as_str())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tasks SET status = ?, finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') WHERE run_id = ? AND status IN (?, ?, ?)")
            .bind(TaskStatus::Cancelled.as_str())
            .bind(run_id)
            .bind(TaskStatus::Pending.as_str())
            .bind(TaskStatus::Dispatched.as_str())
            .bind(TaskStatus::Running.as_str())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn get_run_task_stats_impl(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let row = sqlx::query(
            r#"SELECT COUNT(*) as total,
            COUNT(CASE WHEN status IN (?, ?) THEN 1 END) as completed,
            COUNT(CASE WHEN status = ? THEN 1 END) as failed
            FROM tasks WHERE run_id = ?"#,
        )
        .bind(TaskStatus::Success.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(TaskStatus::Failed.as_str())
        .bind(run_id)
        .fetch_one(&self.pool)
        .await?;
        let total: i64 = row.get("total");
        let completed: i64 = row.get("completed");
        let failed: i64 = row.get("failed");
        Ok((total, completed, failed))
    }
}
