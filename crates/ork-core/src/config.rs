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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_default_config_has_sane_values() {
        let config = OrchestratorConfig::default();

        assert!(config.poll_interval_secs.is_finite());
        assert!(config.poll_interval_secs > 0.0);
        assert!(config.max_tasks_per_batch > 0);
        assert!(config.max_concurrent_dispatches > 0);
        assert!(config.max_concurrent_status_checks > 0);
        assert!(config.db_pool_size > 0);
    }

    #[test]
    fn test_optimized_profile_values() {
        let config = OrchestratorConfig::optimized();

        assert_eq!(config.poll_interval_secs, 2.0);
        assert_eq!(config.max_tasks_per_batch, 50);
        assert_eq!(config.max_concurrent_dispatches, 5);
        assert_eq!(config.max_concurrent_status_checks, 20);
        assert_eq!(config.db_pool_size, 5);
        assert!(config.enable_triggerer);
    }

    #[test]
    fn test_high_throughput_profile_values() {
        let config = OrchestratorConfig::high_throughput();

        assert_eq!(config.poll_interval_secs, 5.0);
        assert_eq!(config.max_tasks_per_batch, 500);
        assert_eq!(config.max_concurrent_dispatches, 50);
        assert_eq!(config.max_concurrent_status_checks, 100);
        assert_eq!(config.db_pool_size, 20);
        assert!(config.enable_triggerer);
    }

    #[test]
    fn test_default_config_reads_env_overrides_and_fallbacks() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = [
            (
                "POLL_INTERVAL_SECS",
                std::env::var("POLL_INTERVAL_SECS").ok(),
            ),
            (
                "MAX_TASKS_PER_BATCH",
                std::env::var("MAX_TASKS_PER_BATCH").ok(),
            ),
            (
                "MAX_CONCURRENT_DISPATCHES",
                std::env::var("MAX_CONCURRENT_DISPATCHES").ok(),
            ),
            (
                "MAX_CONCURRENT_STATUS_CHECKS",
                std::env::var("MAX_CONCURRENT_STATUS_CHECKS").ok(),
            ),
            ("DB_POOL_SIZE", std::env::var("DB_POOL_SIZE").ok()),
            ("ENABLE_TRIGGERER", std::env::var("ENABLE_TRIGGERER").ok()),
        ];

        unsafe {
            std::env::set_var("POLL_INTERVAL_SECS", "0.25");
            std::env::set_var("MAX_TASKS_PER_BATCH", "200");
            std::env::set_var("MAX_CONCURRENT_DISPATCHES", "20");
            std::env::set_var("MAX_CONCURRENT_STATUS_CHECKS", "70");
            std::env::set_var("DB_POOL_SIZE", "15");
            std::env::set_var("ENABLE_TRIGGERER", "false");
        }
        let config = OrchestratorConfig::default();
        assert_eq!(config.poll_interval_secs, 0.25);
        assert_eq!(config.max_tasks_per_batch, 200);
        assert_eq!(config.max_concurrent_dispatches, 20);
        assert_eq!(config.max_concurrent_status_checks, 70);
        assert_eq!(config.db_pool_size, 15);
        assert!(!config.enable_triggerer);

        unsafe {
            std::env::set_var("POLL_INTERVAL_SECS", "not-a-number");
            std::env::set_var("MAX_TASKS_PER_BATCH", "bad");
        }
        let fallback = OrchestratorConfig::default();
        assert_eq!(fallback.poll_interval_secs, 1.0);
        assert_eq!(fallback.max_tasks_per_batch, 100);

        for (key, value) in previous {
            match value {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}
