use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use ork_core::database::Database;
use ork_web::api::{ApiServer, build_router};
use std::sync::Arc;

#[cfg(feature = "postgres")]
use ork_state::PostgresDatabase;
#[cfg(feature = "sqlite")]
use ork_state::SqliteDatabase;

#[cfg(feature = "postgres")]
const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/orchestrator";

#[cfg(feature = "postgres")]
fn default_database_url() -> String {
    DEFAULT_DATABASE_URL.to_string()
}

#[cfg(feature = "sqlite")]
fn default_database_url() -> String {
    if let Ok(home) = std::env::var("HOME") {
        return format!("sqlite://{home}/.ork/ork.db?mode=rwc");
    }
    "sqlite://./.ork/ork.db?mode=rwc".to_string()
}

#[derive(Parser, Debug)]
#[command(name = "ork-web", about = "Serve the Ork web UI and API")]
struct Args {
    /// Database connection string
    #[arg(long, default_value_t = default_database_url())]
    database_url: String,
    /// Address to bind (e.g., 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
}

fn resolve_database_url(default_url: String) -> String {
    std::env::var("DATABASE_URL").unwrap_or(default_url)
}

fn parse_addr(addr: &str) -> SocketAddr {
    addr.parse().expect("invalid address")
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()))
        .try_init();
}

async fn run_api_server<S>(
    db: Arc<dyn Database>,
    addr: SocketAddr,
    shutdown: S,
) -> anyhow::Result<()>
where
    S: std::future::Future<Output = ()> + Send + 'static,
{
    let api = ApiServer::new(db);
    let app = build_router(api);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

async fn run_from_args<S>(args: Args, shutdown: S) -> anyhow::Result<()>
where
    S: std::future::Future<Output = ()> + Send + 'static,
{
    let addr: SocketAddr = parse_addr(&args.addr);
    let database_url = resolve_database_url(args.database_url);

    #[cfg(all(feature = "postgres", not(feature = "sqlite")))]
    let db = Arc::new(PostgresDatabase::new(&database_url).await?);

    #[cfg(all(feature = "sqlite", not(feature = "postgres")))]
    let db = Arc::new(SqliteDatabase::new(&database_url).await?);

    #[cfg(all(feature = "postgres", feature = "sqlite"))]
    compile_error!("Enable either the 'postgres' or 'sqlite' feature for ork-web, not both.");

    #[cfg(not(any(feature = "postgres", feature = "sqlite")))]
    {
        return Err("No database backend enabled. Enable 'postgres' or 'sqlite' feature.".into());
    }

    let db: Arc<dyn Database> = db;
    println!("Serving on http://{}", addr);
    run_api_server(db, addr, shutdown).await
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    run_from_args(args, futures::future::pending::<()>()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;
    use std::sync::{Mutex, OnceLock};
    use tokio::sync::oneshot;
    use tokio::time::{Duration, sleep};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn free_local_addr() -> SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
        listener.local_addr().expect("local addr")
    }

    #[test]
    fn test_args_defaults() {
        let args = Args::parse_from(["ork-web"]);
        assert_eq!(args.database_url, default_database_url());
        assert_eq!(args.addr, "127.0.0.1:8080");
    }

    #[test]
    fn test_resolve_database_url_env_override_and_fallback() {
        let _guard = env_lock().lock().expect("env lock");
        let prev = std::env::var("DATABASE_URL").ok();
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://env-db");
        }
        assert_eq!(
            resolve_database_url("postgres://default-db".to_string()),
            "postgres://env-db"
        );

        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
        assert_eq!(
            resolve_database_url("postgres://default-db".to_string()),
            "postgres://default-db"
        );

        match prev {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
    }

    #[test]
    fn test_parse_addr() {
        let addr = parse_addr("127.0.0.1:4000");
        assert_eq!(addr, "127.0.0.1:4000".parse::<SocketAddr>().expect("addr"));
    }

    #[test]
    #[should_panic(expected = "invalid address")]
    fn test_parse_addr_panics_on_invalid_input() {
        let _ = parse_addr("definitely-not-an-address");
    }

    #[tokio::test]
    async fn test_run_api_server_returns_error_when_bind_fails() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
        db.run_migrations().await.expect("migrations");
        let db: Arc<dyn Database> = db;

        let addr = free_local_addr();
        let _occupied = std::net::TcpListener::bind(addr).expect("occupy addr");
        let result = run_api_server(db, addr, async {}).await;
        assert!(result.is_err(), "bind on occupied addr should fail");
    }

    #[tokio::test]
    async fn test_run_api_server_serves_ui() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("sqlite db"));
        db.run_migrations().await.expect("migrations");
        let db: Arc<dyn Database> = db;

        let addr = free_local_addr();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            run_api_server(db, addr, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let response = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match reqwest::get(format!("http://{addr}/")).await {
                    Ok(resp) => break resp,
                    Err(_) => sleep(Duration::from_millis(50)).await,
                }
            }
        })
        .await
        .expect("http timeout");

        assert!(response.status().is_success());
        let body = response.text().await.expect("ui body");
        assert!(body.contains("<html"));

        let _ = shutdown_tx.send(());
        let result = server.await.expect("join server");
        assert!(result.is_ok());
    }

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn test_run_from_args_serves_ui() {
        init_tracing();
        let addr = free_local_addr();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let args = Args {
            database_url: ":memory:".to_string(),
            addr: addr.to_string(),
        };

        let server = tokio::spawn(async move {
            run_from_args(args, async move {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let response = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match reqwest::get(format!("http://{addr}/")).await {
                    Ok(resp) => break resp,
                    Err(_) => sleep(Duration::from_millis(50)).await,
                }
            }
        })
        .await
        .expect("http timeout");
        assert!(response.status().is_success());

        let _ = shutdown_tx.send(());
        let result = server.await.expect("join server");
        assert!(result.is_ok());
    }

    #[cfg(all(feature = "postgres", not(feature = "sqlite")))]
    #[tokio::test]
    async fn test_run_from_args_errors_when_database_cannot_connect() {
        init_tracing();
        let args = Args {
            database_url: "postgres://postgres:postgres@127.0.0.1:1/ork".to_string(),
            addr: free_local_addr().to_string(),
        };

        let result = run_from_args(args, async {}).await;
        assert!(result.is_err(), "expected DB connect failure");
    }
}
