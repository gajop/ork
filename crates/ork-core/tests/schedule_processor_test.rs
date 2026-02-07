use anyhow::Result;
use ork_core::database::Database;
use ork_core::schedule_processor::process_scheduled_triggers;
use ork_state::SqliteDatabase;

async fn setup_db() -> Result<SqliteDatabase> {
    let db = SqliteDatabase::new(":memory:").await?;
    db.run_migrations().await?;
    Ok(db)
}

#[tokio::test]
async fn test_process_scheduled_triggers_returns_zero_when_none_due() -> Result<()> {
    let db = setup_db().await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_handles_invalid_cron() -> Result<()> {
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "invalid_schedule",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("not-a-cron"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 0);

    let runs = db.list_runs(Some(workflow.id)).await?;
    assert!(runs.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_process_scheduled_triggers_creates_run_and_updates_schedule() -> Result<()> {
    let db = setup_db().await?;
    let workflow = db
        .create_workflow(
            "valid_schedule",
            None,
            "job",
            "local",
            "local",
            "process",
            None,
            Some("* * * * * *"),
        )
        .await?;
    db.update_workflow_schedule(workflow.id, workflow.schedule.as_deref(), true)
        .await?;

    let triggered = process_scheduled_triggers(&db).await?;
    assert_eq!(triggered, 1);

    let runs = db.list_runs(Some(workflow.id)).await?;
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].triggered_by, "schedule");

    let refreshed = db.get_workflow_by_id(workflow.id).await?;
    assert!(refreshed.last_scheduled_at.is_some());
    assert!(refreshed.next_scheduled_at.is_some());

    Ok(())
}
