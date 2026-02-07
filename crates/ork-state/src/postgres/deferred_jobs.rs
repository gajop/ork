use anyhow::Result;
use ork_core::models::DeferredJob;
use sqlx::types::Json;
use uuid::Uuid;

use super::PostgresDatabase;

impl PostgresDatabase {
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
            VALUES ($1, $2, $3, $4, 'pending')
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
            WHERE task_id = $1
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
            SET status = $1,
                error = $2,
                started_at = CASE WHEN started_at IS NULL AND $1 = 'polling' THEN NOW() ELSE started_at END
            WHERE id = $3
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_deferred_job_polled(&self, job_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE deferred_jobs
            SET last_polled_at = NOW()
            WHERE id = $1
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
                finished_at = NOW(),
                started_at = CASE WHEN started_at IS NULL THEN NOW() ELSE started_at END
            WHERE id = $1
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
                error = $1,
                finished_at = NOW(),
                started_at = CASE WHEN started_at IS NULL THEN NOW() ELSE started_at END
            WHERE id = $2
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
                finished_at = NOW()
            WHERE task_id = $1
              AND status IN ('pending', 'polling')
            "#,
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
