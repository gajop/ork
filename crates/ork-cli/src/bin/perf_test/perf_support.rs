use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{PerfConfig, SchedulerConfig};
use crate::perf_metrics::MetricsSummary;

pub fn load_config_from_path(path: &Path) -> Result<PerfConfig> {
    let config_content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", path.display(), e))?;
    serde_yaml::from_str::<PerfConfig>(&config_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config file {}: {}", path.display(), e))
}

pub fn load_config_by_name(name: &str) -> Result<PerfConfig> {
    let config_path = PathBuf::from(format!("perf-configs/{}.yaml", name));
    load_config_from_path(&config_path)
}

pub fn resolve_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/orchestrator".to_string())
}

pub fn resolve_ork_binary() -> String {
    std::env::var("ORK_PERF_ORK_BIN")
        .unwrap_or_else(|_| "../../target/release/ork-cloud-run".to_string())
}

pub fn scheduler_boot_wait() -> Duration {
    match std::env::var("ORK_PERF_BOOT_WAIT_SECS") {
        Ok(raw) => match raw.parse::<f64>() {
            Ok(secs) if secs.is_finite() && secs >= 0.0 => Duration::from_secs_f64(secs),
            _ => Duration::from_secs(2),
        },
        Err(_) => Duration::from_secs(2),
    }
}

pub fn monitor_max_polls() -> Option<usize> {
    let raw = std::env::var("ORK_PERF_MONITOR_MAX_POLLS").ok()?;
    let parsed = raw.parse::<usize>().ok()?;
    (parsed > 0).then_some(parsed)
}

pub fn ensure_perf_task_script(duration: f32) -> Result<()> {
    std::fs::create_dir_all("test-scripts")?;
    std::fs::write(
        "test-scripts/perf-task.sh",
        format!("#!/bin/bash\nsleep {}\n", duration),
    )?;
    std::process::Command::new("chmod")
        .args(["+x", "test-scripts/perf-task.sh"])
        .status()?;
    Ok(())
}

pub fn write_scheduler_config(scheduler: &SchedulerConfig) -> Result<String> {
    let scheduler_config_path = format!("/tmp/ork-scheduler-{}.yaml", std::process::id());
    let scheduler_config_yaml = serde_yaml::to_string(scheduler)?;
    std::fs::write(&scheduler_config_path, scheduler_config_yaml)?;
    Ok(scheduler_config_path)
}

pub fn throughput(
    workflows: u32,
    total_tasks: u32,
    trigger_duration: f64,
    total_duration: f64,
) -> (f64, f64) {
    let task_throughput = total_tasks as f64 / total_duration;
    let run_submission_rate = workflows as f64 / trigger_duration;
    (run_submission_rate, task_throughput)
}

pub fn scheduler_breakdown_lines(summary: MetricsSummary) -> Vec<String> {
    vec![
        "Scheduler Time Breakdown:".to_string(),
        format!("  Total scheduler loops: {}", summary.loops),
        format!(
            "  Time processing runs: {:.2}s ({:.1}%)",
            summary.total_runs_ms as f64 / 1000.0,
            summary.runs_pct()
        ),
        format!(
            "  Time processing tasks: {:.2}s ({:.1}%)",
            summary.total_tasks_ms as f64 / 1000.0,
            summary.tasks_pct()
        ),
        format!(
            "  Time processing status updates: {:.2}s ({:.1}%)",
            summary.total_status_updates_ms as f64 / 1000.0,
            summary.status_updates_pct()
        ),
        format!(
            "  Time sleeping: {:.2}s ({:.1}%)",
            summary.total_sleep_ms as f64 / 1000.0,
            summary.sleep_pct()
        ),
        format!(
            "  Total scheduler time: {:.2}s",
            summary.total_loop_ms as f64 / 1000.0
        ),
    ]
}

pub fn latency_lines(latency_stats: (i64, Option<f64>, Option<f64>, Option<f64>)) -> Vec<String> {
    if let (tasks, Some(avg), Some(min), Some(max)) = latency_stats {
        vec![
            "Latency Stats:".to_string(),
            format!("  Tasks: {}", tasks),
            format!("  Avg latency: {:.3}s", avg),
            format!("  Min latency: {:.3}s", min),
            format!("  Max latency: {:.3}s", max),
        ]
    } else {
        Vec::new()
    }
}

pub fn throughput_lines(
    run_submission_rate: f64,
    task_throughput: f64,
    total_duration: f64,
    workflows: u32,
    tasks_per_workflow: u32,
    total_tasks: u32,
) -> Vec<String> {
    vec![
        "Throughput:".to_string(),
        format!("  Run submission: {:.2} runs/sec", run_submission_rate),
        format!("  Task completion: {:.2} tasks/sec", task_throughput),
        format!("  Total duration: {:.2}s", total_duration),
        "Explanation:".to_string(),
        format!(
            "  - {} runs created ({} tasks each = {} total tasks)",
            workflows, tasks_per_workflow, total_tasks
        ),
        "  - Run submission measures how fast we enqueue work".to_string(),
        "  - Task completion measures actual work throughput".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn sample_config() -> PerfConfig {
        PerfConfig {
            workflows: 2,
            tasks_per_workflow: 3,
            duration: 0.5,
            scheduler: SchedulerConfig {
                poll_interval_secs: 0.1,
                max_tasks_per_batch: 100,
                max_concurrent_dispatches: 4,
                max_concurrent_status_checks: 8,
                db_pool_size: 10,
            },
        }
    }

    #[test]
    fn test_load_config_from_path_success_and_error() {
        let dir = std::env::temp_dir().join(format!("ork-perf-support-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("cfg.yaml");

        let cfg = sample_config();
        std::fs::write(&path, serde_yaml::to_string(&cfg).expect("serialize cfg"))
            .expect("write cfg");
        let loaded = load_config_from_path(&path).expect("load config");
        assert_eq!(loaded.workflows, 2);
        assert_eq!(loaded.scheduler.max_tasks_per_batch, 100);

        let missing = load_config_from_path(&dir.join("missing.yaml"));
        assert!(missing.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_load_config_by_name_reads_from_perf_configs_dir() {
        let name = format!("test-{}", Uuid::new_v4());
        let path = std::path::PathBuf::from(format!("perf-configs/{}.yaml", name));
        let cfg = sample_config();
        std::fs::write(&path, serde_yaml::to_string(&cfg).expect("serialize cfg"))
            .expect("write cfg");

        let loaded = load_config_by_name(&name).expect("load by name");
        assert_eq!(loaded.workflows, 2);
        assert_eq!(loaded.tasks_per_workflow, 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_resolve_database_url_prefers_env() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://original");
        }
        let previous = std::env::var("DATABASE_URL").ok();

        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://env-db");
        }
        assert_eq!(resolve_database_url(), "postgres://env-db");

        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
        assert!(resolve_database_url().contains("localhost:5432/orchestrator"));

        match previous {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
        assert_eq!(resolve_database_url(), "postgres://original");
    }

    #[test]
    fn test_resolve_ork_binary_prefers_env_and_falls_back() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var("ORK_PERF_ORK_BIN").ok();
        unsafe {
            std::env::set_var("ORK_PERF_ORK_BIN", "/tmp/mock-ork");
        }
        assert_eq!(resolve_ork_binary(), "/tmp/mock-ork");

        unsafe {
            std::env::remove_var("ORK_PERF_ORK_BIN");
        }
        assert!(resolve_ork_binary().contains("ork-cloud-run"));

        match previous {
            Some(v) => unsafe { std::env::set_var("ORK_PERF_ORK_BIN", v) },
            None => unsafe { std::env::remove_var("ORK_PERF_ORK_BIN") },
        }
    }

    #[test]
    fn test_scheduler_boot_wait_reads_env_and_handles_invalid_values() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var("ORK_PERF_BOOT_WAIT_SECS").ok();

        unsafe {
            std::env::set_var("ORK_PERF_BOOT_WAIT_SECS", "0.0");
        }
        assert_eq!(scheduler_boot_wait(), Duration::from_secs(0));

        unsafe {
            std::env::set_var("ORK_PERF_BOOT_WAIT_SECS", "-1");
        }
        assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

        unsafe {
            std::env::set_var("ORK_PERF_BOOT_WAIT_SECS", "not-a-number");
        }
        assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

        unsafe {
            std::env::remove_var("ORK_PERF_BOOT_WAIT_SECS");
        }
        assert_eq!(scheduler_boot_wait(), Duration::from_secs(2));

        match previous {
            Some(v) => unsafe { std::env::set_var("ORK_PERF_BOOT_WAIT_SECS", v) },
            None => unsafe { std::env::remove_var("ORK_PERF_BOOT_WAIT_SECS") },
        }
    }

    #[test]
    fn test_monitor_max_polls_env_parsing() {
        let _guard = env_lock().lock().expect("env lock");
        let previous = std::env::var("ORK_PERF_MONITOR_MAX_POLLS").ok();

        unsafe {
            std::env::set_var("ORK_PERF_MONITOR_MAX_POLLS", "3");
        }
        assert_eq!(monitor_max_polls(), Some(3));

        unsafe {
            std::env::set_var("ORK_PERF_MONITOR_MAX_POLLS", "0");
        }
        assert_eq!(monitor_max_polls(), None);

        unsafe {
            std::env::set_var("ORK_PERF_MONITOR_MAX_POLLS", "bad");
        }
        assert_eq!(monitor_max_polls(), None);

        unsafe {
            std::env::remove_var("ORK_PERF_MONITOR_MAX_POLLS");
        }
        assert_eq!(monitor_max_polls(), None);

        match previous {
            Some(v) => unsafe { std::env::set_var("ORK_PERF_MONITOR_MAX_POLLS", v) },
            None => unsafe { std::env::remove_var("ORK_PERF_MONITOR_MAX_POLLS") },
        }
    }

    #[test]
    fn test_resolve_database_url_restore_some_branch_executes() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
        let previous = std::env::var("DATABASE_URL").ok();
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://during");
        }
        assert_eq!(resolve_database_url(), "postgres://during");
        match previous {
            Some(v) => unsafe { std::env::set_var("DATABASE_URL", v) },
            None => unsafe { std::env::remove_var("DATABASE_URL") },
        }
        assert!(resolve_database_url().contains("localhost:5432/orchestrator"));
    }

    #[test]
    fn test_ensure_perf_task_script_writes_shell_script() {
        ensure_perf_task_script(1.25).expect("create script");
        let content = std::fs::read_to_string("test-scripts/perf-task.sh").expect("read script");
        assert!(content.contains("sleep 1.25"));
    }

    #[test]
    fn test_write_scheduler_config_and_throughput() {
        let cfg = sample_config().scheduler;
        let path = write_scheduler_config(&cfg).expect("write scheduler cfg");
        let content = std::fs::read_to_string(&path).expect("read scheduler cfg");
        assert!(content.contains("poll_interval_secs"));
        assert!(content.contains("max_tasks_per_batch"));

        let (runs, tasks) = throughput(4, 40, 2.0, 8.0);
        assert_eq!(runs, 2.0);
        assert_eq!(tasks, 5.0);

        let summary = MetricsSummary {
            total_runs_ms: 100,
            total_tasks_ms: 200,
            total_status_updates_ms: 300,
            total_sleep_ms: 400,
            total_loop_ms: 1000,
            loops: 4,
            last_timestamp: 123,
        };
        let scheduler_lines = scheduler_breakdown_lines(summary);
        assert!(
            scheduler_lines
                .iter()
                .any(|line| line.contains("Total scheduler loops: 4"))
        );

        let latency = latency_lines((4, Some(1.2), Some(0.8), Some(2.1)));
        assert!(
            latency
                .iter()
                .any(|line| line.contains("Avg latency: 1.200s"))
        );
        assert!(latency_lines((4, None, Some(1.0), Some(2.0))).is_empty());

        let throughput = throughput_lines(2.0, 5.0, 8.0, 4, 10, 40);
        assert!(
            throughput
                .iter()
                .any(|line| line.contains("Run submission: 2.00"))
        );
        assert!(
            throughput
                .iter()
                .any(|line| line.contains("4 runs created (10 tasks each = 40 total tasks)"))
        );

        let _ = std::fs::remove_file(path);
    }
}
