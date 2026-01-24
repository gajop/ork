mod cloud_run;
mod db;
mod models;
mod scheduler;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::db::Database;
use crate::scheduler::Scheduler;

#[derive(Parser)]
#[command(name = "rust-orchestrator")]
#[command(about = "A simple Rust-based orchestrator for Cloud Run jobs", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, default_value = "postgres://postgres:postgres@localhost:5432/orchestrator")]
    database_url: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the database with migrations
    Init,

    /// Start the orchestrator scheduler
    Run,

    /// Create a new workflow
    CreateWorkflow {
        /// Workflow name
        #[arg(short, long)]
        name: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Cloud Run job name
        #[arg(short, long)]
        job_name: String,

        /// Cloud Run project ID
        #[arg(short, long)]
        project: String,

        /// Cloud Run region
        #[arg(short, long, default_value = "us-central1")]
        region: String,

        /// Number of tasks to generate per run
        #[arg(short, long, default_value = "3")]
        task_count: i32,
    },

    /// List all workflows
    ListWorkflows,

    /// Trigger a workflow run
    Trigger {
        /// Workflow name to trigger
        workflow_name: String,
    },

    /// Show status of runs
    Status {
        /// Specific run ID to show (optional)
        run_id: Option<String>,

        /// Workflow name to filter by (optional)
        #[arg(short, long)]
        workflow: Option<String>,
    },

    /// Show tasks for a run
    Tasks {
        /// Run ID
        run_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let cli = Cli::parse();

    // Connect to database (prefer DATABASE_URL env var if set)
    let database_url = std::env::var("DATABASE_URL").unwrap_or(cli.database_url);
    let db = Arc::new(Database::new(&database_url).await?);

    match cli.command {
        Commands::Init => {
            info!("Running database migrations...");
            db.run_migrations().await?;
            println!("✓ Database initialized successfully");
        }

        Commands::Run => {
            info!("Starting orchestrator...");
            let scheduler = Scheduler::new(db.clone());
            scheduler.run().await?;
        }

        Commands::CreateWorkflow {
            name,
            description,
            job_name,
            project,
            region,
            task_count,
        } => {
            let task_params = serde_json::json!({
                "task_count": task_count,
            });

            let workflow = db
                .create_workflow(
                    &name,
                    description.as_deref(),
                    &job_name,
                    &region,
                    &project,
                    Some(task_params),
                )
                .await?;

            println!("✓ Created workflow: {}", workflow.name);
            println!("  ID: {}", workflow.id);
            println!("  Cloud Run Job: {}", workflow.cloud_run_job_name);
            println!("  Project: {}", workflow.cloud_run_project);
            println!("  Region: {}", workflow.cloud_run_region);
        }

        Commands::ListWorkflows => {
            let workflows = db.list_workflows().await?;

            if workflows.is_empty() {
                println!("No workflows found");
            } else {
                println!("Workflows:");
                println!("{:<36} {:<30} {:<30}", "ID", "Name", "Cloud Run Job");
                println!("{}", "-".repeat(96));

                for wf in workflows {
                    println!(
                        "{:<36} {:<30} {:<30}",
                        wf.id, wf.name, wf.cloud_run_job_name
                    );
                }
            }
        }

        Commands::Trigger { workflow_name } => {
            let workflow = db.get_workflow(&workflow_name).await?;
            let run = db.create_run(workflow.id, "manual").await?;

            println!("✓ Triggered workflow: {}", workflow_name);
            println!("  Run ID: {}", run.id);
            println!("  Status: {}", run.status);
            println!("\nUse 'status {}' to check progress", run.id);
        }

        Commands::Status { run_id, workflow } => {
            let runs = if let Some(rid) = run_id {
                let run_uuid = rid.parse()?;
                vec![db.get_run(run_uuid).await?]
            } else if let Some(wf_name) = workflow {
                let workflow = db.get_workflow(&wf_name).await?;
                db.list_runs(Some(workflow.id)).await?
            } else {
                db.list_runs(None).await?
            };

            if runs.is_empty() {
                println!("No runs found");
            } else {
                println!("Runs:");
                println!(
                    "{:<36} {:<36} {:<12} {:<20} {:<20}",
                    "Run ID", "Workflow ID", "Status", "Started", "Finished"
                );
                println!("{}", "-".repeat(124));

                for run in runs {
                    let started = run
                        .started_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "-".to_string());

                    let finished = run
                        .finished_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<36} {:<36} {:<12} {:<20} {:<20}",
                        run.id, run.workflow_id, run.status, started, finished
                    );
                }
            }
        }

        Commands::Tasks { run_id } => {
            let run_uuid = run_id.parse()?;
            let tasks = db.list_tasks(run_uuid).await?;

            if tasks.is_empty() {
                println!("No tasks found for run {}", run_id);
            } else {
                println!("Tasks for run {}:", run_id);
                println!(
                    "{:<5} {:<36} {:<12} {:<40} {:<20}",
                    "Index", "Task ID", "Status", "Execution", "Finished"
                );
                println!("{}", "-".repeat(113));

                for task in tasks {
                    let execution = task
                        .cloud_run_execution_name
                        .as_deref()
                        .unwrap_or("-");

                    let finished = task
                        .finished_at
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "-".to_string());

                    println!(
                        "{:<5} {:<36} {:<12} {:<40} {:<20}",
                        task.task_index, task.id, task.status, execution, finished
                    );
                }
            }
        }
    }

    Ok(())
}
