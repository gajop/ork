/// Orchestrator configuration for performance tuning
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OrchestratorConfig {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,

    /// Maximum number of tasks to process per batch
    pub max_tasks_per_batch: i64,

    /// Maximum concurrent task dispatches
    pub max_concurrent_dispatches: usize,

    /// Maximum concurrent status checks
    pub max_concurrent_status_checks: usize,

    /// Database connection pool size
    pub db_pool_size: u32,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            max_tasks_per_batch: 100,
            max_concurrent_dispatches: 10,
            max_concurrent_status_checks: 50,
            db_pool_size: 10,
        }
    }
}

impl OrchestratorConfig {
    /// Create a low-latency, low-memory configuration
    #[allow(dead_code)]
    pub fn optimized() -> Self {
        Self {
            poll_interval_secs: 2,         // Faster polling
            max_tasks_per_batch: 50,        // Smaller batches
            max_concurrent_dispatches: 5,   // Limited concurrency
            max_concurrent_status_checks: 20, // Moderate status checks
            db_pool_size: 5,                // Minimal connections
        }
    }

    /// Create a high-throughput configuration
    #[allow(dead_code)]
    pub fn high_throughput() -> Self {
        Self {
            poll_interval_secs: 5,
            max_tasks_per_batch: 500,
            max_concurrent_dispatches: 50,
            max_concurrent_status_checks: 100,
            db_pool_size: 20,
        }
    }
}
