use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::VecDeque;
use std::io::{BufReader, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::sleep;

#[cfg(test)]
#[path = "perf_test/integration_tests.rs"]
mod integration_tests;
#[path = "perf_test/perf_metrics.rs"]
mod perf_metrics;
#[path = "perf_test/perf_support.rs"]
mod perf_support;

#[derive(Parser)]
#[command(name = "perf-test")]
#[command(about = "Performance testing tool for ork-cloud-run")]
struct Args {
    /// Config file to load (from perf-configs/*.yaml)
    #[arg(short, long)]
    config: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct PerfConfig {
    workflows: u32,
    tasks_per_workflow: u32,
    duration: f32,
    scheduler: SchedulerConfig,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct SchedulerConfig {
    poll_interval_secs: f64,
    max_tasks_per_batch: i64,
    max_concurrent_dispatches: usize,
    max_concurrent_status_checks: usize,
    #[serde(default = "default_db_pool_size")]
    db_pool_size: u32,
}

fn default_db_pool_size() -> u32 {
    10
}

#[derive(Debug)]
struct ResourceStats {
    scheduler_rss_kb: u64,
    scheduler_cpu_percent: f32,
}

#[derive(Debug, Deserialize)]
struct SchedulerMetrics {
    timestamp: u64,
    process_pending_runs_ms: u128,
    process_pending_tasks_ms: u128,
    process_status_updates_ms: u128,
    sleep_ms: u128,
    total_loop_ms: u128,
}

async fn run(args: Args) -> Result<()> {
    let config = perf_support::load_config_by_name(&args.config)?;
    let database_url = perf_support::resolve_database_url();
    let ork_bin = perf_support::resolve_ork_binary();

    let pool = PgPool::connect(&database_url).await?;

    println!("=== Performance Test Configuration ===");
    println!("Config: {}", args.config);
    println!("Runs to trigger: {}", config.workflows);
    println!("Tasks per run: {}", config.tasks_per_workflow);
    println!("Task duration: {}s", config.duration);
    println!(
        "Total tasks: {}",
        config.workflows * config.tasks_per_workflow
    );
    println!();

    // Create test script
    println!("Creating test script...");
    perf_support::ensure_perf_task_script(config.duration)?;

    // Clean old workflow
    println!("Creating performance test workflow...");
    sqlx::query(
        r#"
        DELETE FROM tasks
        WHERE run_id IN (
            SELECT r.id
            FROM runs r
            JOIN workflows w ON r.workflow_id = w.id
            WHERE w.name = 'perf-test'
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM runs
        WHERE workflow_id IN (
            SELECT id FROM workflows WHERE name = 'perf-test'
        )
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query("DELETE FROM workflows WHERE name = 'perf-test'")
        .execute(&pool)
        .await?;

    // Create workflow
    let create_status = Command::new(&ork_bin)
        .args([
            "create-workflow",
            "--name",
            "perf-test",
            "--description",
            "Performance test workflow",
            "--job-name",
            "perf-task.sh",
            "--project",
            "local",
            "--region",
            "local",
            "--task-count",
            &config.tasks_per_workflow.to_string(),
            "--executor",
            "process",
        ])
        .status()?;

    if !create_status.success() {
        anyhow::bail!("Failed to create workflow");
    }

    // Write scheduler config to temp file
    let scheduler_config_path = perf_support::write_scheduler_config(&config.scheduler)?;

    // Start scheduler in background with config
    println!("Starting scheduler...");
    println!("  Poll interval: {}s", config.scheduler.poll_interval_secs);
    println!("  Max batch: {}", config.scheduler.max_tasks_per_batch);
    println!(
        "  Max concurrent dispatches: {}",
        config.scheduler.max_concurrent_dispatches
    );
    println!(
        "  Max concurrent status checks: {}",
        config.scheduler.max_concurrent_status_checks
    );

    // Redirect scheduler logs to file so we can read them
    let scheduler_log_path = format!("/tmp/ork-scheduler-{}.log", std::process::id());
    let log_file = std::fs::File::create(&scheduler_log_path)?;

    let mut scheduler = Command::new(&ork_bin)
        .args(["run", "--config", &scheduler_config_path])
        .env("RUST_LOG", "info")
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()?;

    let scheduler_pid = Pid::from_u32(scheduler.id());
    sleep(perf_support::scheduler_boot_wait()).await;

    // Get initial memory stats
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
    let scheduler_rss_start = system
        .process(scheduler_pid)
        .map(|p| p.memory() / 1024) // Convert bytes to KB
        .unwrap_or(0);

    // Trigger workflows
    println!();
    println!("=== Starting Performance Test ===");
    let start_time = Instant::now();

    let mut trigger_handles = Vec::new();
    for _ in 0..config.workflows {
        let ork_bin = ork_bin.clone();
        let handle = tokio::spawn(async move {
            Command::new(&ork_bin)
                .args(["trigger", "perf-test"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
        });
        trigger_handles.push(handle);
    }

    for handle in trigger_handles {
        handle.await??;
    }

    let trigger_end = Instant::now();
    let trigger_duration = trigger_end.duration_since(start_time).as_secs_f64();

    println!("All runs triggered");

    // Monitor completion
    println!("Monitoring task completion...");
    let total_tasks = config.workflows * config.tasks_per_workflow;
    let mut completed = 0u32;
    let mut all_metrics: VecDeque<SchedulerMetrics> = VecDeque::new();
    let mut log_position: u64 = 0;
    let max_monitor_polls = perf_support::monitor_max_polls();
    let mut monitor_polls = 0usize;

    while completed < total_tasks {
        sleep(Duration::from_millis(500)).await;
        monitor_polls += 1;

        // Get task counts by status
        let status_counts: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM tasks GROUP BY status")
                .fetch_all(&pool)
                .await?;
        let counts = perf_metrics::status_counts_from_rows(status_counts);
        completed = (counts.success + counts.failed) as u32;

        // Get resource stats
        system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
        let resource_stats = if let Some(process) = system.process(scheduler_pid) {
            ResourceStats {
                scheduler_rss_kb: process.memory() / 1024, // Convert bytes to KB
                scheduler_cpu_percent: process.cpu_usage(),
            }
        } else {
            ResourceStats {
                scheduler_rss_kb: 0,
                scheduler_cpu_percent: 0.0,
            }
        };

        let elapsed = start_time.elapsed().as_secs_f64();

        // Read new lines from log file since last position
        if let Ok(mut file) = std::fs::File::open(&scheduler_log_path)
            && file.seek(SeekFrom::Start(log_position)).is_ok()
        {
            let reader = BufReader::new(&file);
            perf_metrics::append_metrics_from_reader(reader, &mut all_metrics);
            // Update position to current end of file
            if let Ok(metadata) = std::fs::metadata(&scheduler_log_path) {
                log_position = metadata.len();
            }
        }

        // Show cumulative metrics with both absolute values and percentages
        let metrics_display = perf_metrics::format_metrics_display(&all_metrics);

        println!(
            "[{:.1}s] Completed:{}/{} | Pending:{} Dispatched:{} Running:{} | Scheduler: {} KB, {:.1}% CPU | {}",
            elapsed,
            completed,
            total_tasks,
            counts.pending,
            counts.dispatched,
            counts.running,
            resource_stats.scheduler_rss_kb,
            resource_stats.scheduler_cpu_percent,
            metrics_display
        );

        if let Some(max_polls) = max_monitor_polls
            && monitor_polls >= max_polls
            && completed < total_tasks
        {
            println!(
                "Stopping monitor after {} polls before completion (ORK_PERF_MONITOR_MAX_POLLS={})",
                monitor_polls, max_polls
            );
            break;
        }
    }

    let end_time = Instant::now();
    let total_duration = end_time.duration_since(start_time).as_secs_f64();

    println!();
    println!();
    println!("=== Performance Test Results ===");

    // Analyze scheduler metrics
    if let Some(summary) = perf_metrics::summarize_metrics(&all_metrics) {
        println!();
        for line in perf_support::scheduler_breakdown_lines(summary) {
            println!("{line}");
        }
    }

    // Query latency stats
    let latency_stats: (i64, Option<f64>, Option<f64>, Option<f64>) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as tasks,
            AVG(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as avg_latency_sec,
            MIN(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as min_latency_sec,
            MAX(EXTRACT(EPOCH FROM (finished_at - created_at)))::FLOAT8 as max_latency_sec
        FROM tasks
        WHERE finished_at IS NOT NULL
        "#,
    )
    .fetch_one(&pool)
    .await?;

    let latency_lines = perf_support::latency_lines(latency_stats);
    if !latency_lines.is_empty() {
        println!();
        for line in latency_lines {
            println!("{line}");
        }
    }

    // Calculate throughput
    let (run_submission_rate, task_throughput) = perf_support::throughput(
        config.workflows,
        total_tasks,
        trigger_duration,
        total_duration,
    );

    println!();
    for line in perf_support::throughput_lines(
        run_submission_rate,
        task_throughput,
        total_duration,
        config.workflows,
        config.tasks_per_workflow,
        total_tasks,
    ) {
        println!("{line}");
    }

    // Final resource stats
    system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
    let scheduler_rss = system
        .process(scheduler_pid)
        .map(|p| p.memory() / 1024) // Convert bytes to KB
        .unwrap_or(0);

    println!();
    println!("Resource Usage:");
    println!("  Scheduler RSS: {} KB", scheduler_rss);
    println!(
        "  Scheduler RSS growth: {} KB",
        scheduler_rss.saturating_sub(scheduler_rss_start)
    );

    // Cleanup
    println!();
    println!("Cleaning up...");
    let _ = scheduler.kill();
    let _ = std::fs::remove_file(&scheduler_config_path);
    let _ = std::fs::remove_file(&scheduler_log_path);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    run(Args::parse()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_db_pool_size() {
        assert_eq!(default_db_pool_size(), 10);
    }

    #[test]
    fn test_perf_config_deserialization() {
        let yaml = r#"
workflows: 2
tasks_per_workflow: 3
duration: 0.5
scheduler:
  poll_interval_secs: 0.1
  max_tasks_per_batch: 100
  max_concurrent_dispatches: 10
  max_concurrent_status_checks: 20
"#;

        let cfg: PerfConfig = serde_yaml::from_str(yaml).expect("config should deserialize");
        assert_eq!(cfg.workflows, 2);
        assert_eq!(cfg.tasks_per_workflow, 3);
        assert_eq!(cfg.scheduler.db_pool_size, 10);
    }

    #[test]
    fn test_scheduler_metrics_deserialization() {
        let value = serde_json::json!({
            "timestamp": 123,
            "process_pending_runs_ms": 10,
            "process_pending_tasks_ms": 20,
            "process_status_updates_ms": 30,
            "sleep_ms": 40,
            "total_loop_ms": 100
        });
        let parsed: SchedulerMetrics =
            serde_json::from_value(value).expect("metrics should deserialize");
        assert_eq!(parsed.process_pending_runs_ms, 10);
        assert_eq!(parsed.total_loop_ms, 100);
    }
}
