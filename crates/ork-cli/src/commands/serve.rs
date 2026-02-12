use anyhow::Result;
use clap::Args;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

use ork_core::config::OrchestratorConfig;
use ork_core::database::Database;
use ork_core::scheduler::Scheduler;
use ork_executors::ExecutorManager;
use ork_web::api::{ApiServer, build_router};

#[derive(Args)]
pub struct Serve {
    /// Optional config file path (YAML)
    #[arg(short, long)]
    pub config: Option<String>,

    /// Address to bind the web UI (e.g., 127.0.0.1:4000)
    #[arg(long, default_value = "127.0.0.1:4000")]
    pub addr: String,
}

impl Serve {
    pub async fn execute<D>(self, db: Arc<D>) -> Result<()>
    where
        D: Database + 'static,
    {
        let addr: SocketAddr = self.addr.parse()?;

        // Initialize database if needed
        db.run_migrations().await?;

        info!("Starting orchestrator...");
        let executor_manager = Arc::new(ExecutorManager::new());
        let scheduler = if let Some(config_path) = self.config {
            let config_content = std::fs::read_to_string(&config_path)?;
            let orchestrator_config: OrchestratorConfig = serde_yaml::from_str(&config_content)?;
            Scheduler::new_with_config(db.clone(), executor_manager, orchestrator_config)
        } else {
            Scheduler::new(db.clone(), executor_manager)
        };

        // Start web server in background
        let web_db = db.clone();
        let web_handle = tokio::spawn(async move {
            let api = ApiServer::new(web_db);
            let app = build_router(api);
            let listener = tokio::net::TcpListener::bind(addr).await?;
            println!("Web UI available at http://{}", addr);
            axum::serve(listener, app).await?;
            Ok::<(), anyhow::Error>(())
        });

        // Run scheduler in main task
        let scheduler_result = scheduler.run().await;

        // If scheduler exits, stop web server
        web_handle.abort();

        scheduler_result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;

    fn free_local_addr() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
        listener.local_addr().expect("local addr").to_string()
    }

    #[tokio::test]
    async fn test_serve_command_errors_on_missing_config_file() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let result = Serve {
            config: Some("/tmp/does-not-exist-config.yaml".to_string()),
            addr: free_local_addr(),
        }
        .execute(db)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_serve_command_errors_on_invalid_yaml_config() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let path = std::env::temp_dir().join(format!("ork-serve-invalid-{}.yaml", Uuid::new_v4()));
        std::fs::write(&path, "this: [is: not-valid-yaml").expect("write invalid yaml");

        let result = Serve {
            config: Some(path.to_string_lossy().to_string()),
            addr: free_local_addr(),
        }
        .execute(db)
        .await;

        let _ = std::fs::remove_file(path);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_serve_command_uses_default_config() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let handle = tokio::spawn(async move {
            Serve {
                config: None,
                addr: free_local_addr(),
            }
            .execute(db)
            .await
        });
        sleep(Duration::from_millis(100)).await;
        handle.abort();
        let join = handle.await;
        assert!(join.is_err(), "aborted serve should cancel task");
    }

    #[tokio::test]
    async fn test_serve_command_uses_config_file() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let path = std::env::temp_dir().join(format!("ork-serve-valid-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"
poll_interval_secs: 0.01
max_tasks_per_batch: 10
max_concurrent_dispatches: 2
max_concurrent_status_checks: 2
db_pool_size: 2
enable_triggerer: false
"#,
        )
        .expect("write valid yaml");

        let cfg_path = path.to_string_lossy().to_string();
        let handle = tokio::spawn(async move {
            Serve {
                config: Some(cfg_path),
                addr: free_local_addr(),
            }
            .execute(db)
            .await
        });
        sleep(Duration::from_millis(100)).await;
        handle.abort();
        let join = handle.await;
        let _ = std::fs::remove_file(path);
        assert!(join.is_err(), "aborted serve should cancel task");
    }

    #[tokio::test]
    async fn test_serve_command_starts_web_server() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        db.run_migrations().await.expect("migrate");

        let addr = free_local_addr();
        let test_addr = addr.clone();

        let handle = tokio::spawn(async move {
            Serve {
                config: None,
                addr,
            }
            .execute(db)
            .await
        });

        // Wait for server to start
        sleep(Duration::from_millis(200)).await;

        // Verify web server is responding
        let response = reqwest::get(format!("http://{}/", test_addr))
            .await
            .expect("web request");
        assert!(response.status().is_success() || response.status().is_redirection());

        handle.abort();
    }
}
