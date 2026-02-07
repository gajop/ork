use serde::Deserialize;

/// Orchestrator configuration for performance tuning
#[derive(Debug, Clone, Deserialize)]
pub struct OrchestratorConfig {
    /// Polling interval in seconds (supports fractional seconds like 0.5)
    pub poll_interval_secs: f64,

    /// Maximum number of tasks to process per batch
    pub max_tasks_per_batch: i64,

    /// Maximum concurrent task dispatches
    pub max_concurrent_dispatches: usize,

    /// Maximum concurrent status checks
    pub max_concurrent_status_checks: usize,

    /// Database connection pool size
    pub db_pool_size: u32,

    /// Enable deferred-job triggerer startup.
    pub enable_triggerer: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: std::env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
            max_tasks_per_batch: std::env::var("MAX_TASKS_PER_BATCH")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            max_concurrent_dispatches: std::env::var("MAX_CONCURRENT_DISPATCHES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            max_concurrent_status_checks: std::env::var("MAX_CONCURRENT_STATUS_CHECKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            db_pool_size: std::env::var("DB_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            enable_triggerer: std::env::var("ENABLE_TRIGGERER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}

impl OrchestratorConfig {
    /// Create a low-latency, low-memory configuration
    pub fn optimized() -> Self {
        Self {
            poll_interval_secs: 2.0,          // Faster polling
            max_tasks_per_batch: 50,          // Smaller batches
            max_concurrent_dispatches: 5,     // Limited concurrency
            max_concurrent_status_checks: 20, // Moderate status checks
            db_pool_size: 5,                  // Minimal connections
            enable_triggerer: true,
        }
    }

    /// Create a high-throughput configuration
    pub fn high_throughput() -> Self {
        Self {
            poll_interval_secs: 5.0,
            max_tasks_per_batch: 500,
            max_concurrent_dispatches: 50,
            max_concurrent_status_checks: 100,
            db_pool_size: 20,
            enable_triggerer: true,
        }
    }
}
