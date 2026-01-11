use std::{collections::HashMap, path::PathBuf, time::Instant};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum LocalTaskState {
    Pending,
    Running {
        attempt: u32,
    },
    Success {
        attempt: u32,
        output: Option<serde_json::Value>,
    },
    Failed {
        attempt: u32,
        error: String,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct RunSummary {
    pub run_id: String,
    #[serde(skip)]
    pub started_at: Instant,
    #[serde(skip)]
    pub finished_at: Instant,
    pub statuses: HashMap<String, LocalTaskState>,
    pub run_dir: PathBuf,
}
