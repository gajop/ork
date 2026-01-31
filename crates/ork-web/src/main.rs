mod api;

use std::net::SocketAddr;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use api::ApiServer;
use std::sync::Arc;

#[cfg(feature = "postgres")]
use ork_state::PostgresDatabase;
#[cfg(not(feature = "postgres"))]
use ork_core::database::Database;

#[derive(Parser, Debug)]
#[command(name = "ork-web", about = "Serve the Ork web UI and API")]
struct Args {
    /// Database connection string
    #[arg(
        long,
        default_value = "postgres://postgres:postgres@localhost:5432/orchestrator"
    )]
    database_url: String,
    /// Address to bind (e.g., 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or(args.database_url);

    #[cfg(feature = "postgres")]
    let db = Arc::new(PostgresDatabase::new(&database_url).await?);

    #[cfg(not(feature = "postgres"))]
    let db: Arc<dyn Database> = {
        return Err("No database backend enabled. Enable 'postgres' feature.".into());
    };

    let server = ApiServer::new(db);
    let addr: SocketAddr = args.addr.parse().expect("invalid address");
    server.serve(addr).await;
    println!("Serving on http://{}", addr);
    futures::future::pending::<()>().await;
    Ok(())
}
