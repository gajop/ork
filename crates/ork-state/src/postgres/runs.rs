use super::core::PostgresDatabase;
use anyhow::Result;
use chrono::{DateTime, Utc};
use ork_core::database::{RunListEntry, RunListPage, RunListQuery};
use ork_core::models::{Run, RunStatus, TaskStatus};
use sqlx::Row;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct RunWithWorkflowRow {
    id: Uuid,
    workflow_id: Uuid,
    snapshot_id: Option<Uuid>,
    status: RunStatus,
    triggered_by: String,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    workflow_name: Option<String>,
}

impl PostgresDatabase {
    pub(super) async fn create_run_impl(
        &self,
        workflow_id: Uuid,
        triggered_by: &str,
    ) -> Result<Run> {
        // Get workflow's current snapshot
        let workflow = self.get_workflow_by_id_impl(workflow_id).await?;
        let snapshot_id = workflow.current_snapshot_id;

        let run = sqlx::query_as::<_, Run>(
            "INSERT INTO runs (workflow_id, snapshot_id, status, triggered_by) VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(workflow_id)
        .bind(snapshot_id)
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

    pub(super) async fn list_runs_page_impl(&self, query: &RunListQuery) -> Result<RunListPage> {
        let limit = i64::try_from(query.limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(query.offset).unwrap_or(i64::MAX);
        let status = query.status.map(|s| s.as_str().to_string());

        let (total, rows) = match (status.as_deref(), query.workflow_name.as_deref()) {
            (Some(status), Some(workflow_name)) => {
                let total = sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*)
                       FROM runs r
                       INNER JOIN workflows w ON w.id = r.workflow_id
                       WHERE r.status = $1 AND w.name = $2"#,
                )
                .bind(status)
                .bind(workflow_name)
                .fetch_one(&self.pool)
                .await?;

                let rows = sqlx::query_as::<_, RunWithWorkflowRow>(
                    r#"SELECT r.id, r.workflow_id, r.snapshot_id, r.status, r.triggered_by, r.started_at, r.finished_at, r.error, r.created_at,
                              w.name AS workflow_name
                       FROM runs r
                       LEFT JOIN workflows w ON w.id = r.workflow_id
                       WHERE r.status = $1 AND w.name = $2
                       ORDER BY r.created_at DESC, r.id DESC
                       LIMIT $3 OFFSET $4"#,
                )
                .bind(status)
                .bind(workflow_name)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, rows)
            }
            (Some(status), None) => {
                let total =
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs WHERE status = $1")
                        .bind(status)
                        .fetch_one(&self.pool)
                        .await?;

                let rows = sqlx::query_as::<_, RunWithWorkflowRow>(
                    r#"SELECT r.id, r.workflow_id, r.snapshot_id, r.status, r.triggered_by, r.started_at, r.finished_at, r.error, r.created_at,
                              w.name AS workflow_name
                       FROM runs r
                       LEFT JOIN workflows w ON w.id = r.workflow_id
                       WHERE r.status = $1
                       ORDER BY r.created_at DESC, r.id DESC
                       LIMIT $2 OFFSET $3"#,
                )
                .bind(status)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, rows)
            }
            (None, Some(workflow_name)) => {
                let total = sqlx::query_scalar::<_, i64>(
                    r#"SELECT COUNT(*)
                       FROM runs r
                       INNER JOIN workflows w ON w.id = r.workflow_id
                       WHERE w.name = $1"#,
                )
                .bind(workflow_name)
                .fetch_one(&self.pool)
                .await?;

                let rows = sqlx::query_as::<_, RunWithWorkflowRow>(
                    r#"SELECT r.id, r.workflow_id, r.snapshot_id, r.status, r.triggered_by, r.started_at, r.finished_at, r.error, r.created_at,
                              w.name AS workflow_name
                       FROM runs r
                       LEFT JOIN workflows w ON w.id = r.workflow_id
                       WHERE w.name = $1
                       ORDER BY r.created_at DESC, r.id DESC
                       LIMIT $2 OFFSET $3"#,
                )
                .bind(workflow_name)
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, rows)
            }
            (None, None) => {
                let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
                    .fetch_one(&self.pool)
                    .await?;

                let rows = sqlx::query_as::<_, RunWithWorkflowRow>(
                    r#"SELECT r.id, r.workflow_id, r.snapshot_id, r.status, r.triggered_by, r.started_at, r.finished_at, r.error, r.created_at,
                              w.name AS workflow_name
                       FROM runs r
                       LEFT JOIN workflows w ON w.id = r.workflow_id
                       ORDER BY r.created_at DESC, r.id DESC
                       LIMIT $1 OFFSET $2"#,
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?;
                (total, rows)
            }
        };

        let items = rows
            .into_iter()
            .map(|row| RunListEntry {
                run: Run {
                    id: row.id,
                    workflow_id: row.workflow_id,
                    snapshot_id: row.snapshot_id,
                    status: row.status,
                    triggered_by: row.triggered_by,
                    started_at: row.started_at,
                    finished_at: row.finished_at,
                    error: row.error,
                    created_at: row.created_at,
                },
                workflow_name: row.workflow_name,
            })
            .collect();

        Ok(RunListPage {
            items,
            total: total as usize,
        })
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
