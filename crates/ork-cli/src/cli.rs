use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ork", about = "Lightweight local ork runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Validate a workflow file
    Validate { workflow: PathBuf },

    /// Execute a workflow locally
    Run {
        workflow: PathBuf,
        /// Maximum tasks to run at once
        #[arg(long, default_value_t = 4)]
        parallel: usize,
        /// Directory to store run artifacts (defaults to .ork/runs)
        #[arg(long)]
        run_dir: Option<PathBuf>,
        /// Directory to store state (defaults to .ork/state)
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Backend used for state metadata (file or memory)
        #[arg(long, value_enum, default_value = "file")]
        state_backend: crate::StateBackend,
    },

    /// Show the latest run status for a workflow from persisted state
    Status {
        /// Workflow run id (if not provided, shows most recent run)
        #[arg(long)]
        run_id: Option<String>,
        /// Base artifact dir (defaults to .ork/runs)
        #[arg(long)]
        run_dir: Option<PathBuf>,
        /// Directory to read state from (defaults to .ork/state)
        #[arg(long)]
        state_dir: Option<PathBuf>,
        /// Max runs to list when no run_id is provided
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Also show task outputs when printing a run
        #[arg(long, default_value_t = false)]
        show_outputs: bool,
        /// Backend used for state metadata (file or memory)
        #[arg(long, value_enum, default_value = "file")]
        state_backend: crate::StateBackend,
    },
}
