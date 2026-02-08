use anyhow::Result;
use chrono::Utc;
use cron;
use std::str::FromStr;
use tracing::{error, info};

use crate::database::Database;
use crate::models::Workflow;

/// Process workflows that have schedules enabled and are due for triggering
pub async fn process_scheduled_triggers<D: Database>(db: &D) -> Result<usize> {
    let workflows = db.get_due_scheduled_workflows().await?;

    if workflows.is_empty() {
        return Ok(0);
    }

    let now = Utc::now();
    let mut triggered_count = 0;

    for workflow in workflows {
        if let Some(schedule_str) = workflow.schedule.as_deref() {
            triggered_count += process_workflow_schedule(db, &workflow, schedule_str, now).await;
        }
    }

    if triggered_count > 0 {
        info!("Triggered {} scheduled workflow runs", triggered_count);
    }

    Ok(triggered_count)
}

async fn process_workflow_schedule<D: Database>(
    db: &D,
    workflow: &Workflow,
    schedule_str: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> usize {
    // Parse cron expression
    let schedule = match cron::Schedule::from_str(schedule_str) {
        Ok(s) => s,
        Err(e) => {
            error!(
                "Invalid cron expression '{}' for workflow {}: {}",
                schedule_str, workflow.name, e
            );
            return 0;
        }
    };

    // Create run for this scheduled trigger
    match db.create_run(workflow.id, "schedule").await {
        Ok(run) => {
            info!(
                "Created scheduled run {} for workflow {}",
                run.id, workflow.name
            );
        }
        Err(e) => {
            error!(
                "Failed to create scheduled run for workflow {}: {}",
                workflow.name, e
            );
            return 0;
        }
    }

    // Calculate next scheduled time
    let next_scheduled_at = schedule.after(&now).next();

    // Update workflow schedule times
    if let Err(e) = db
        .update_workflow_schedule_times(workflow.id, now, next_scheduled_at)
        .await
    {
        error!(
            "Failed to update schedule times for workflow {}: {}",
            workflow.name, e
        );
    }

    1
}
