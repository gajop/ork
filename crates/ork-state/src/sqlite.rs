use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use uuid::Uuid;

use ork_core::database::Database as DatabaseTrait;
use ork_core::database::{NewTask, NewWorkflowTask};
use ork_core::models::{Run, Task, TaskWithWorkflow, Workflow, WorkflowTask};

fn parse_depends_on(raw: Option<String>) -> Vec<String> {
    match raw {
        Some(value) => serde_json::from_str(&value).unwrap_or_default(),
        None => Vec::new(),
    }
}

fn encode_depends_on(deps: &[String]) -> String {
    serde_json::to_string(deps).unwrap_or_else(|_| "[]".to_string())
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: Uuid,
    run_id: Uuid,
    task_index: i32,
    task_name: String,
    executor_type: String,
    depends_on: String,
    status: String,
    execution_name: Option<String>,
    params: Option<sqlx::types::Json<serde_json::Value>>,
    output: Option<sqlx::types::Json<serde_json::Value>>,
    logs: Option<String>,
    error: Option<String>,
    dispatched_at: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct WorkflowTaskRow {
    id: Uuid,
    workflow_id: Uuid,
    task_index: i32,
    task_name: String,
    executor_type: String,
    depends_on: String,
    params: Option<sqlx::types::Json<serde_json::Value>>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct TaskWithWorkflowRow {
    task_id: Uuid,
    run_id: Uuid,
    task_index: i32,
    task_name: String,
    executor_type: String,
    depends_on: String,
    task_status: String,
    execution_name: Option<String>,
    params: Option<sqlx::types::Json<serde_json::Value>>,
    workflow_id: Uuid,
    job_name: String,
    project: String,
    region: String,
}

pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(database_url)
            .await?;
        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations_sqlite").run(&self.pool).await?;
        Ok(())
    }

    fn map_task(row: TaskRow) -> Task {
        Task {
            id: row.id,
            run_id: row.run_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            status: row.status,
            execution_name: row.execution_name,
            params: row.params.map(|v| v),
            output: row.output.map(|v| v),
            logs: row.logs,
            error: row.error,
            dispatched_at: row.dispatched_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            created_at: row.created_at,
        }
    }

    fn map_workflow_task(row: WorkflowTaskRow) -> WorkflowTask {
        WorkflowTask {
            id: row.id,
            workflow_id: row.workflow_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            params: row.params.map(|v| v),
            created_at: row.created_at,
        }
    }

    fn map_task_with_workflow(row: TaskWithWorkflowRow) -> TaskWithWorkflow {
        TaskWithWorkflow {
            task_id: row.task_id,
            run_id: row.run_id,
            task_index: row.task_index,
            task_name: row.task_name,
            executor_type: row.executor_type,
            depends_on: parse_depends_on(Some(row.depends_on)),
            task_status: row.task_status,
            execution_name: row.execution_name,
            params: row.params.map(|v| v),
            workflow_id: row.workflow_id,
            job_name: row.job_name,
            project: row.project,
            region: row.region,
        }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl DatabaseTrait for SqliteDatabase {
    async fn run_migrations(&self) -> Result<()> {
        SqliteDatabase::run_migrations(self).await
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
        let workflow_id = Uuid::new_v4();
        let params_json = task_params.map(|v| sqlx::types::Json(v));
        let workflow = sqlx::query_as::<_, Workflow>(
            r#"
            INSERT INTO workflows (id, name, description, job_name, region, project, executor_type, task_params)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(workflow_id)
        .bind(name)
        .bind(description)
        .bind(job_name)
        .bind(region)
        .bind(project)
        .bind(executor_type)
        .bind(params_json)
        .fetch_one(&self.pool)
        .await?;
        Ok(workflow)
    }

    async fn get_workflow(&self, name: &str) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await?;
        Ok(workflow)
    }

    async fn list_workflows(&self) -> Result<Vec<Workflow>> {
        let workflows = sqlx::query_as::<_, Workflow>(
            "SELECT * FROM workflows ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(workflows)
    }

    async fn delete_workflow(&self, name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "DELETE FROM runs WHERE workflow_id = (SELECT id FROM workflows WHERE name = ?)",
        )
        .bind(name)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM workflows WHERE name = ?")
            .bind(name)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn create_run(&self, workflow_id: Uuid, triggered_by: &str) -> Result<Run> {
        let run_id = Uuid::new_v4();
        let run = sqlx::query_as::<_, Run>(
            r#"
            INSERT INTO runs (id, workflow_id, status, triggered_by)
            VALUES (?, ?, 'pending', ?)
            RETURNING *
            "#,
        )
        .bind(run_id)
        .bind(workflow_id)
        .bind(triggered_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(run)
    }

    async fn update_run_status(
        &self,
        run_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE runs
            SET status = ?,
                error = ?,
                started_at = COALESCE(started_at, CASE WHEN ? = 'running' THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END),
                finished_at = CASE WHEN ? IN ('success', 'failed', 'cancelled') THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(status)
        .bind(status)
        .bind(run_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_run(&self, run_id: Uuid) -> Result<Run> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(run)
    }

    async fn list_runs(&self, workflow_id: Option<Uuid>) -> Result<Vec<Run>> {
        let runs = if let Some(wf_id) = workflow_id {
            sqlx::query_as::<_, Run>(
                "SELECT * FROM runs WHERE workflow_id = ? ORDER BY created_at DESC LIMIT 50",
            )
            .bind(wf_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC LIMIT 50")
                .fetch_all(&self.pool)
                .await?
        };
        Ok(runs)
    }

    async fn get_pending_runs(&self) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            "SELECT * FROM runs WHERE status = 'pending' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(runs)
    }

    async fn batch_create_tasks(
        &self,
        run_id: Uuid,
        task_count: i32,
        workflow_name: &str,
        executor_type: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for i in 0..task_count {
            let params = serde_json::json!({
                "task_index": i,
                "run_id": run_id.to_string(),
                "workflow_name": workflow_name,
            });
            let params = sqlx::types::Json(params);
            let depends_on = encode_depends_on(&[]);

            sqlx::query(
                r#"
                INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, params)
                VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(run_id)
            .bind(i)
            .bind(format!("task_{}", i))
            .bind(executor_type)
            .bind(depends_on)
            .bind(params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn batch_create_dag_tasks(&self, run_id: Uuid, tasks: &[NewTask]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for task in tasks {
            let params = sqlx::types::Json(task.params.clone());
            let depends_on = encode_depends_on(&task.depends_on);
            sqlx::query(
                r#"
                INSERT INTO tasks (id, run_id, task_index, task_name, executor_type, depends_on, status, params)
                VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(run_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(depends_on)
            .bind(params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn create_workflow_tasks(
        &self,
        workflow_id: Uuid,
        tasks: &[NewWorkflowTask],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM workflow_tasks WHERE workflow_id = ?")
            .bind(workflow_id)
            .execute(&mut *tx)
            .await?;

        for task in tasks {
            let params = sqlx::types::Json(task.params.clone());
            let depends_on = encode_depends_on(&task.depends_on);
            sqlx::query(
                r#"
                INSERT INTO workflow_tasks (id, workflow_id, task_index, task_name, executor_type, depends_on, params)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(workflow_id)
            .bind(task.task_index)
            .bind(&task.task_name)
            .bind(&task.executor_type)
            .bind(depends_on)
            .bind(params)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_workflow_tasks(&self, workflow_id: Uuid) -> Result<Vec<WorkflowTask>> {
        let rows = sqlx::query_as::<_, WorkflowTaskRow>(
            "SELECT * FROM workflow_tasks WHERE workflow_id = ? ORDER BY task_index ASC",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Self::map_workflow_task).collect())
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: &str,
        execution_name: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = ?,
                execution_name = COALESCE(?, execution_name),
                error = ?,
                dispatched_at = COALESCE(dispatched_at, CASE WHEN ? = 'dispatched' THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END),
                started_at = COALESCE(started_at, CASE WHEN ? = 'running' THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END),
                finished_at = CASE WHEN ? IN ('success', 'failed', 'cancelled') THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(execution_name)
        .bind(error)
        .bind(status)
        .bind(status)
        .bind(status)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn batch_update_task_status(
        &self,
        updates: &[(Uuid, &str, Option<&str>, Option<&str>)],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;

        for (task_id, status, execution_name, error) in updates {
            sqlx::query(
                r#"
                UPDATE tasks
                SET status = ?,
                    execution_name = COALESCE(?, execution_name),
                    error = ?,
                    dispatched_at = COALESCE(dispatched_at, CASE WHEN ? = 'dispatched' THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END),
                    started_at = COALESCE(started_at, CASE WHEN ? = 'running' THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END),
                    finished_at = CASE WHEN ? IN ('success', 'failed', 'cancelled') THEN STRFTIME('%Y-%m-%dT%H:%M:%fZ','now') END
                WHERE id = ?
                  AND (status NOT IN ('success', 'failed') OR ? IN ('success', 'failed'))
                "#,
            )
            .bind(status)
            .bind(execution_name)
            .bind(error)
            .bind(status)
            .bind(status)
            .bind(status)
            .bind(task_id)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_tasks(&self, run_id: Uuid) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE run_id = ? ORDER BY task_index ASC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    async fn get_pending_tasks(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE status = 'pending' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    async fn get_running_tasks(&self) -> Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE status IN ('dispatched', 'running') ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Self::map_task).collect())
    }

    async fn append_task_log(&self, task_id: Uuid, chunk: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tasks
            SET logs = COALESCE(logs, '') || ?
            WHERE id = ?
            "#,
        )
        .bind(chunk)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_pending_tasks_with_workflow(
        &self,
        limit: i64,
    ) -> Result<Vec<TaskWithWorkflow>> {
        let rows = sqlx::query_as::<_, TaskWithWorkflowRow>(
            r#"
            SELECT
                t.id as task_id,
                t.run_id,
                t.task_index,
                t.task_name,
                t.executor_type,
                t.depends_on,
                t.status as task_status,
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
            ORDER BY t.created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let run_ids: Vec<Uuid> = rows.iter().map(|r| r.run_id).collect();
        let mut qb = QueryBuilder::<Sqlite>::new(
            "SELECT run_id, task_name, status FROM tasks WHERE run_id IN (",
        );
        let mut separated = qb.separated(", ");
        for run_id in run_ids {
            separated.push_bind(run_id);
        }
        qb.push(")");
        let dep_rows = qb.build().fetch_all(&self.pool).await?;
        let mut status_map: std::collections::HashMap<(Uuid, String), String> =
            std::collections::HashMap::new();
        for row in dep_rows {
            let run_id: Uuid = row.try_get(0)?;
            let task_name: String = row.try_get(1)?;
            let status: String = row.try_get(2)?;
            status_map.insert((run_id, task_name), status);
        }

        let mut out = Vec::new();
        for row in rows {
            let deps = parse_depends_on(Some(row.depends_on.clone()));
            let ready = deps.iter().all(|name| {
                status_map
                    .get(&(row.run_id, name.clone()))
                    .map(|s| s == "success")
                    .unwrap_or(false)
            });
            if deps.is_empty() || ready {
                out.push(Self::map_task_with_workflow(row));
                if out.len() as i64 >= limit {
                    break;
                }
            }
        }

        Ok(out)
    }

    async fn get_workflows_by_ids(&self, workflow_ids: &[Uuid]) -> Result<Vec<Workflow>> {
        if workflow_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM workflows WHERE id IN (");
        let mut separated = qb.separated(", ");
        for id in workflow_ids {
            separated.push_bind(*id);
        }
        qb.push(")");
        let workflows = qb
            .build_query_as::<Workflow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(workflows)
    }

    async fn get_workflow_by_id(&self, workflow_id: Uuid) -> Result<Workflow> {
        let workflow = sqlx::query_as::<_, Workflow>("SELECT * FROM workflows WHERE id = ?")
            .bind(workflow_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(workflow)
    }

    async fn get_task_run_id(&self, task_id: Uuid) -> Result<Uuid> {
        let (run_id,): (Uuid,) =
            sqlx::query_as("SELECT run_id FROM tasks WHERE id = ?")
                .bind(task_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(run_id)
    }

    async fn get_task_identity(&self, task_id: Uuid) -> Result<(Uuid, String)> {
        let (run_id, task_name): (Uuid, String) =
            sqlx::query_as("SELECT run_id, task_name FROM tasks WHERE id = ?")
                .bind(task_id)
                .fetch_one(&self.pool)
                .await?;
        Ok((run_id, task_name))
    }

    async fn mark_tasks_failed_by_dependency(
        &self,
        run_id: Uuid,
        failed_task_names: &[String],
        error: &str,
    ) -> Result<Vec<String>> {
        if failed_task_names.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE run_id = ? AND status = 'pending'",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;

        let mut tx = self.pool.begin().await?;
        let mut updated = Vec::new();

        for row in rows {
            let deps = parse_depends_on(Some(row.depends_on));
            if deps
                .iter()
                .any(|dep| failed_task_names.iter().any(|f| f == dep))
            {
                sqlx::query(
                    r#"
                    UPDATE tasks
                    SET status = 'failed',
                        error = ?,
                        finished_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ','now')
                    WHERE id = ?
                    "#,
                )
                .bind(error)
                .bind(row.id)
                .execute(&mut *tx)
                .await?;
                updated.push(row.task_name);
            }
        }

        tx.commit().await?;
        Ok(updated)
    }

    async fn get_run_task_stats(&self, run_id: Uuid) -> Result<(i64, i64, i64)> {
        let stats: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as total,
                SUM(CASE WHEN status IN ('success', 'failed') THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed
            FROM tasks
            WHERE run_id = ?
            "#,
        )
        .bind(run_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(stats)
    }
}
