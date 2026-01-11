mod api;

use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use api::ApiServer;

#[derive(Parser, Debug)]
#[command(name = "ork-web", about = "Serve the Ork web UI and API")]
struct Args {
    /// Base artifact dir (defaults to .ork/runs)
    #[arg(long)]
    run_dir: Option<PathBuf>,
    /// Directory to read/write state (defaults to .ork/state)
    #[arg(long)]
    state_dir: Option<PathBuf>,
    /// Address to bind (e.g., 127.0.0.1:8080)
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
    /// Max tasks to run at once (affects scheduler interactions)
    #[arg(long, default_value_t = 4)]
    parallel: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let base_dir = args.run_dir.unwrap_or_else(|| PathBuf::from(".ork/runs"));
    let state_base = args
        .state_dir
        .unwrap_or_else(|| PathBuf::from(".ork/state"));
    std::fs::create_dir_all(&base_dir).ok();
    std::fs::create_dir_all(&state_base).ok();

    let server = ApiServer::new(&state_base, &base_dir, args.parallel);
    let addr: SocketAddr = args.addr.parse().expect("invalid address");
    server.serve(addr).await;
    println!("Serving on http://{}", addr);
    futures::future::pending::<()>().await;
    Ok(())
}
