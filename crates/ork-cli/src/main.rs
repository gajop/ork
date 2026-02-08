mod commands;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use commands::Commands;
use ork_core::database::Database;

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

fn resolve_database_url(default_url: String) -> String {
    std::env::var("DATABASE_URL").unwrap_or(default_url)
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into());
    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        // Tests may initialize tracing multiple times; it's fine once a global
        // subscriber is already installed.
        return Ok(());
    }
    Ok(())
}

async fn handle_api_command(command: Commands) -> Result<Option<Commands>> {
    if command.uses_api()
        && let Commands::RunWorkflow(cmd) = command
    {
        cmd.execute().await?;
        return Ok(None);
    }
    Ok(Some(command))
}

async fn dispatch_command<D>(command: Commands, db: Arc<D>) -> Result<()>
where
    D: Database + 'static,
{
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

async fn run_cli(cli: Cli) -> Result<()> {
    let Cli {
        command,
        database_url,
    } = cli;

    let Some(command) = handle_api_command(command).await? else {
        return Ok(());
    };

    let database_url = resolve_database_url(database_url);

    let db = Arc::new(Db::new(&database_url).await?);
    dispatch_command(command, db).await
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;
    run_cli(Cli::parse()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        CreateWorkflow, CreateWorkflowYaml, DeleteWorkflow, Execute, Init, ListWorkflows, Run,
        RunWorkflow, Status, Tasks, Trigger,
    };
    use ork_state::SqliteDatabase;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_cli_parse_defaults() {
        let cli = Cli::parse_from(["ork", "init"]);
        assert_eq!(
            cli.database_url,
            "postgres://postgres:postgres@localhost:5432/orchestrator"
        );
    }

    #[test]
    fn test_resolve_database_url_prefers_env_var() {
        let _guard = env_lock().lock().expect("env lock");
        let prev = std::env::var("DATABASE_URL").ok();
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://env-db");
        }
        let resolved = resolve_database_url("postgres://default-db".to_string());
        assert_eq!(resolved, "postgres://env-db");

        match prev {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
    }

    #[test]
    fn test_resolve_database_url_falls_back_to_default() {
        let _guard = env_lock().lock().expect("env lock");
        let prev = std::env::var("DATABASE_URL").ok();
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
        let resolved = resolve_database_url("postgres://default-db".to_string());
        assert_eq!(resolved, "postgres://default-db");

        if let Some(v) = prev {
            unsafe {
                std::env::set_var("DATABASE_URL", v);
            }
        }
    }

    async fn setup_db() -> Arc<SqliteDatabase> {
        let db = Arc::new(
            SqliteDatabase::new(":memory:")
                .await
                .expect("create sqlite db"),
        );
        db.run_migrations().await.expect("migrate");
        db
    }

    #[tokio::test]
    async fn test_handle_api_command_for_non_api_command() {
        let command = Commands::Init(Init);
        let handled = handle_api_command(command)
            .await
            .expect("handle non-api command");
        assert!(matches!(handled, Some(Commands::Init(_))));
    }

    #[tokio::test]
    async fn test_handle_api_command_executes_run_workflow() {
        let command = Commands::RunWorkflow(RunWorkflow {
            file: "/tmp/does-not-exist-workflow.yaml".to_string(),
            api_url: "http://127.0.0.1:4000".to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        });
        let err = handle_api_command(command)
            .await
            .err()
            .expect("missing file should error");
        assert!(err.to_string().contains("Missing workflow file"));
    }

    #[tokio::test]
    async fn test_dispatch_command_variants() {
        let db = setup_db().await;

        dispatch_command(Commands::Init(Init), db.clone())
            .await
            .expect("init");

        dispatch_command(
            Commands::CreateWorkflow(CreateWorkflow {
                name: "wf-main".to_string(),
                description: Some("desc".to_string()),
                job_name: "job".to_string(),
                project: "local".to_string(),
                region: "local".to_string(),
                task_count: 1,
                executor: "process".to_string(),
            }),
            db.clone(),
        )
        .await
        .expect("create workflow");

        let yaml_path = std::env::temp_dir().join(format!("ork-main-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &yaml_path,
            r#"
name: wf-main-yaml
tasks:
  step_a:
    executor: process
    command: "echo a"
"#,
        )
        .expect("write yaml");
        dispatch_command(
            Commands::CreateWorkflowYaml(CreateWorkflowYaml {
                file: yaml_path.to_string_lossy().to_string(),
                project: "local".to_string(),
                region: "local".to_string(),
            }),
            db.clone(),
        )
        .await
        .expect("create workflow yaml");

        dispatch_command(Commands::ListWorkflows(ListWorkflows), db.clone())
            .await
            .expect("list workflows");

        dispatch_command(
            Commands::Trigger(Trigger {
                workflow_name: "wf-main".to_string(),
            }),
            db.clone(),
        )
        .await
        .expect("trigger");

        dispatch_command(
            Commands::Status(Status {
                run_id: None,
                workflow: Some("wf-main".to_string()),
            }),
            db.clone(),
        )
        .await
        .expect("status");

        let run = db
            .list_runs(None)
            .await
            .expect("list runs")
            .into_iter()
            .next()
            .expect("run exists");
        dispatch_command(
            Commands::Tasks(Tasks {
                run_id: run.id.to_string(),
            }),
            db.clone(),
        )
        .await
        .expect("tasks");

        dispatch_command(
            Commands::DeleteWorkflow(DeleteWorkflow {
                workflow_name: "wf-main".to_string(),
            }),
            db.clone(),
        )
        .await
        .expect("delete workflow");

        let _ = std::fs::remove_file(yaml_path);
    }

    #[tokio::test]
    async fn test_dispatch_command_run_and_execute_error_paths() {
        let db = setup_db().await;

        let run_err = dispatch_command(
            Commands::Run(Run {
                config: Some("/tmp/does-not-exist-config.yaml".to_string()),
            }),
            db.clone(),
        )
        .await
        .expect_err("missing run config should error");
        assert!(
            run_err.to_string().contains("No such file")
                || run_err.to_string().contains("os error")
        );

        let exec_err = dispatch_command(
            Commands::Execute(Execute {
                file: "/tmp/does-not-exist-workflow.yaml".to_string(),
                config: None,
                timeout: 1,
            }),
            db,
        )
        .await
        .expect_err("missing execute file should error");
        assert!(
            exec_err
                .to_string()
                .contains("Failed to read workflow file")
        );
    }

    #[tokio::test]
    async fn test_run_cli_db_connect_error_path() {
        let cli = Cli {
            command: Commands::Init(Init),
            database_url: "postgres://postgres:postgres@127.0.0.1:1/ork".to_string(),
        };

        let err = run_cli(cli).await.expect_err("db connect should fail");
        assert!(
            err.to_string().contains("Connection")
                || err.to_string().contains("connection")
                || err.to_string().contains("refused")
        );
    }

    #[tokio::test]
    async fn test_run_cli_api_command_returns_error_before_db_connect() {
        let cli = Cli {
            command: Commands::RunWorkflow(RunWorkflow {
                file: "/tmp/does-not-exist-workflow.yaml".to_string(),
                api_url: "http://127.0.0.1:4000".to_string(),
                project: "local".to_string(),
                region: "local".to_string(),
                root: None,
                replace: true,
            }),
            database_url: "postgres://postgres:postgres@127.0.0.1:1/ork".to_string(),
        };

        let err = run_cli(cli)
            .await
            .expect_err("missing workflow should fail before db init");
        assert!(err.to_string().contains("Missing workflow file"));
    }
}
