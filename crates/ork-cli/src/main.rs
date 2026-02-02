mod commands;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use commands::Commands;

#[cfg(feature = "postgres")]
use ork_state::PostgresDatabase;
#[cfg(feature = "sqlite")]
use ork_state::SqliteDatabase;

#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
type Db = PostgresDatabase;

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
type Db = SqliteDatabase;

#[cfg(all(feature = "postgres", feature = "sqlite"))]
compile_error!("Enable either the 'postgres' or 'sqlite' feature for ork-cli, not both.");

#[cfg(not(any(feature = "postgres", feature = "sqlite")))]
compile_error!("Enable either the 'postgres' or 'sqlite' feature for ork-cli.");

#[derive(Parser)]
#[command(name = "ork")]
#[command(about = "Ork - A high-performance task orchestrator supporting multiple execution backends", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(
        long,
        default_value = "postgres://postgres:postgres@localhost:5432/orchestrator"
    )]
    database_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into());
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let Cli {
        command,
        database_url,
    } = Cli::parse();

    if command.uses_api() {
        if let Commands::RunWorkflow(cmd) = command {
            return cmd.execute().await;
        }
    }

    let database_url = std::env::var("DATABASE_URL").unwrap_or(database_url);

    let db = Arc::new(Db::new(&database_url).await?);

    match command {
        Commands::Init(cmd) => cmd.execute(db).await?,
        Commands::Run(cmd) => cmd.execute(db).await?,
        Commands::CreateWorkflow(cmd) => cmd.execute(db).await?,
        Commands::CreateWorkflowYaml(cmd) => cmd.execute(db).await?,
        Commands::ListWorkflows(cmd) => cmd.execute(db).await?,
        Commands::DeleteWorkflow(cmd) => cmd.execute(db).await?,
        Commands::Trigger(cmd) => cmd.execute(db).await?,
        Commands::Status(cmd) => cmd.execute(db).await?,
        Commands::Tasks(cmd) => cmd.execute(db).await?,
        Commands::Execute(cmd) => cmd.execute(db).await?,
        Commands::RunWorkflow(_) => unreachable!("run-workflow handled earlier"),
    }

    Ok(())
}
