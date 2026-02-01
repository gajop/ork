use anyhow::Result;
use uuid::Uuid;

use ork_core::models::Run;

use super::core::SqliteDatabase;

impl SqliteDatabase {
    pub(super) async fn create_run_impl(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        let run_id = Uuid::new_v4();
        let run = sqlx::query_as::<_, Run>(
            r#"INSERT INTO runs (id, workflow_id, status, triggered_by) VALUES (?, ?, 'pending', ?) RETURNING *"#,
        )
        .bind(run_id).bind(workflow_id).bind(triggered_by).fetch_one(&self.pool).await?;
        Ok(run)
    }

    pub(super) async fn update_run_status_impl(&self, run_id: Uuid, status: &str, error: Option<&str>) -> Result<()> {
        sqlx::query(
            r#"UPDATE runs SET status = ?, error = ?,
            started_at = CASE WHEN ? = 'running' AND started_at IS NULL THEN CURRENT_TIMESTAMP ELSE started_at END,
            finished_at = CASE WHEN ? IN ('success', 'failed', 'cancelled') AND finished_at IS NULL THEN CURRENT_TIMESTAMP ELSE finished_at END
            WHERE id = ?"#,
        )
        .bind(status).bind(error).bind(status).bind(status).bind(run_id).execute(&self.pool).await?;
        Ok(())
    }

    pub(super) async fn get_run_impl(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = ?")
            .bind(run_id).fetch_one(&self.pool).await?;
        Ok(run)
    }

    pub(super) async fn list_runs_impl(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = match workflow_id {
            Some(wf_id) => {
                sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE workflow_id = ? ORDER BY created_at DESC")
                    .bind(wf_id).fetch_all(&self.pool).await?
            }
            None => {
                sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC")
                    .fetch_all(&self.pool).await?
            }
        };
        Ok(runs)
    }

    pub(super) async fn get_pending_runs_impl(&self) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE status = 'pending'")
            .fetch_all(&self.pool).await?;
        Ok(runs)
    }

    pub(super) async fn cancel_run_impl(&self, run_id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE runs SET status = 'cancelled', finished_at = CURRENT_TIMESTAMP WHERE id = ? AND status NOT IN ('success', 'failed', 'cancelled')")
            .bind(run_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE tasks SET status = 'cancelled', finished_at = CURRENT_TIMESTAMP WHERE run_id = ? AND status IN ('pending', 'dispatched', 'running')")
            .bind(run_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn get_run_task_stats_impl(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let row = sqlx::query(
            r#"SELECT COUNT(*) as total,
            COUNT(CASE WHEN status IN ('success', 'failed') THEN 1 END) as completed,
            COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed
            FROM tasks WHERE run_id = ?"#,
        )
        .bind(run_id).fetch_one(&self.pool).await?;
        let total: i64 = row.get("total");
        let completed: i64 = row.get("completed");
        let failed: i64 = row.get("failed");
        Ok((total, completed, failed))
    }
}
