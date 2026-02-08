use super::core::PostgresDatabase;
use anyhow::Result;
use ork_core::models::{Run, RunStatus, TaskStatus};
use sqlx::Row;
use uuid::Uuid;

impl PostgresDatabase {
    pub(super) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>(
            "INSERT INTO runs (workflow_id, status, triggered_by) VALUES ($1, $2, $3) RETURNING *",
        )
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
        sqlx::query("UPDATE runs SET status = $1, error = $2, started_at = COALESCE(started_at, CASE WHEN $1 = $3 THEN NOW() ELSE NULL END), finished_at = CASE WHEN $1 IN ($4, $5, $6) THEN NOW() ELSE NULL END WHERE id = $7")
            .bind(status_str)
            .bind(error)
            .bind(RunStatus::Running.as_str())
            .bind(RunStatus::Success.as_str())
            .bind(RunStatus::Failed.as_str())
            .bind(RunStatus::Cancelled.as_str())
            .bind(run_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub(super) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(run)
    }
    pub(super) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = match workflow_id {
            Some(wf_id) => {
                sqlx::query_as::<_, Run>(
                    "SELECT * FROM runs WHERE workflow_id = $1 ORDER BY created_at DESC",
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
        let runs = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE status = $1")
            .bind(RunStatus::Pending.as_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(runs)
    }
    pub(super) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE runs SET status = $1, finished_at = NOW() WHERE id = $2 AND status NOT IN ($3, $4, $5)",
        )
            .bind(RunStatus::Cancelled.as_str())
            .bind(run_id)
            .bind(RunStatus::Success.as_str())
            .bind(RunStatus::Failed.as_str())
            .bind(RunStatus::Cancelled.as_str())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tasks SET status = $1, finished_at = NOW() WHERE run_id = $2 AND status IN ($3, $4, $5)")
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
        let row = sqlx::query("SELECT COUNT(*) as total, COUNT(*) FILTER (WHERE status IN ($1, $2)) as completed, COUNT(*) FILTER (WHERE status = $2) as failed FROM tasks WHERE run_id = $3")
            .bind(TaskStatus::Success.as_str())
            .bind(TaskStatus::Failed.as_str())
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok((row.get("total"), row.get("completed"), row.get("failed")))
    }
}
