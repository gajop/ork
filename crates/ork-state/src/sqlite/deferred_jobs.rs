use anyhow::Result;
use ork_core::models::DeferredJob;
use sqlx::types::Json;
use uuid::Uuid;

use super::SqliteDatabase;

impl SqliteDatabase {
    pub async fn create_deferred_job(
        &self,
        task_id: Uuid,
        service_type: &str,
        job_id: &str,
        job_data: serde_json::Value,
    ) -> Result<DeferredJob> {
        let job = sqlx::query_as::<_, DeferredJob>(
            r#"
            INSERT INTO deferred_jobs (task_id, service_type, job_id, job_data, status)
            VALUES (?, ?, ?, ?, 'pending')
            RETURNING *
            "#,
        )
        .bind(task_id)
        .bind(service_type)
        .bind(job_id)
        .bind(Json(job_data))
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn get_pending_deferred_jobs(&self) -> Result<Vec<DeferredJob>> {
        let jobs = sqlx::query_as::<_, DeferredJob>(
            r#"
            SELECT * FROM deferred_jobs
            WHERE status IN ('pending', 'polling')
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn get_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<Vec<DeferredJob>> {
        let jobs = sqlx::query_as::<_, DeferredJob>(
            r#"
            SELECT * FROM deferred_jobs
            WHERE task_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn update_deferred_job_status(
        &self,
        job_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = ?,
                error = ?,
                started_at = CASE WHEN started_at IS NULL AND ? = 'polling' THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(status)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET last_polled_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn complete_deferred_job(&self, job_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = 'completed',
                finished_at = CURRENT_TIMESTAMP,
                started_at = CASE WHEN started_at IS NULL THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn fail_deferred_job(&self, job_id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = 'failed',
                error = ?,
                finished_at = CURRENT_TIMESTAMP,
                started_at = CASE WHEN started_at IS NULL THEN CURRENT_TIMESTAMP ELSE started_at END
            WHERE id = ?
            "#,
        )
        .bind(error)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cancel_deferred_jobs_for_task(&self, task_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET status = 'cancelled',
                finished_at = CURRENT_TIMESTAMP
            WHERE task_id = ?
              AND status IN ('pending', 'polling')
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
