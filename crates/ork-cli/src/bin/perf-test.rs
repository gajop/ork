use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::time::sleep;

#[derive(Parser)]
#[command(name = "perf-test")]
#[command(about = "Performance testing tool for ork-cloud-run")]
struct Args {
    /// Config file to load (from perf-configs/*.yaml)
    #[arg(short, long)]
    config: String,
}

#[derive(Debug, Deserialize)]
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
    #[allow(dead_code)]
    timestamp: u64,
    process_pending_runs_ms: u128,
    process_pending_tasks_ms: u128,
    process_status_updates_ms: u128,
    sleep_ms: u128,
    total_loop_ms: u128,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load config from file
    let config_path = format!("perf-configs/{}.yaml", args.config);
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read config file {}: {}", config_path, e))?;
    let config = serde_yaml::from_str::<PerfConfig>(&config_content)
        .map_err(|e| anyhow::anyhow!("Failed to parse config file {}: {}", config_path, e))?;

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/orchestrator".to_string());

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
    std::fs::create_dir_all("test-scripts")?;
    std::fs::write(
        "test-scripts/perf-task.sh",
        format!("#!/bin/bash\nsleep {}\n", config.duration),
    )?;
    Command::new("chmod")
        .args(["+x", "test-scripts/perf-task.sh"])
        .status()?;

    // Clean old workflow
    println!("Creating performance test workflow...");
    sqlx::query("DELETE FROM workflows WHERE name = 'perf-test'")
        .execute(&pool)
        .await?;

    // Create workflow
    let create_status = Command::new("../../target/release/ork-cloud-run")
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
    let scheduler_config_path = format!("/tmp/ork-scheduler-{}.yaml", std::process::id());
    let scheduler_config_yaml = serde_yaml::to_string(&config.scheduler)?;
    std::fs::write(&scheduler_config_path, scheduler_config_yaml)?;

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

    let mut scheduler = Command::new("../../target/release/ork-cloud-run")
        .args(["run", "--config", &scheduler_config_path])
        .env("RUST_LOG", "info")
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()?;

    let scheduler_pid = Pid::from_u32(scheduler.id());
    sleep(Duration::from_secs(2)).await;

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
        let handle = tokio::spawn(async {
            Command::new("../../target/release/ork-cloud-run")
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

    while completed < total_tasks {
        sleep(Duration::from_millis(500)).await;

        // Get task counts by status
        let status_counts: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM tasks GROUP BY status")
                .fetch_all(&pool)
                .await?;

        let mut pending = 0i64;
        let mut dispatched = 0i64;
        let mut running = 0i64;
        let mut success = 0i64;
        let mut failed = 0i64;

        for (status, count) in status_counts {
            match status.as_str() {
                "pending" => pending = count,
                "dispatched" => dispatched = count,
                "running" => running = count,
                "success" => success = count,
                "failed" => failed = count,
                _ => {}
            }
        }

        completed = (success + failed) as u32;

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
            for line in reader.lines().map_while(Result::ok) {
                if let Some(json_start) = line.find("SCHEDULER_METRICS: ") {
                    let json_str = &line[json_start + "SCHEDULER_METRICS: ".len()..];
                    if let Ok(metrics) = serde_json::from_str::<SchedulerMetrics>(json_str) {
                        all_metrics.push_back(metrics);
                    }
                }
            }
            // Update position to current end of file
            if let Ok(metadata) = std::fs::metadata(&scheduler_log_path) {
                log_position = metadata.len();
            }
        }

        // Show cumulative metrics with both absolute values and percentages
        let metrics_display = if !all_metrics.is_empty() {
            let total_runs: u128 = all_metrics.iter().map(|m| m.process_pending_runs_ms).sum();
            let total_tasks: u128 = all_metrics.iter().map(|m| m.process_pending_tasks_ms).sum();
            let total_status_updates: u128 = all_metrics
                .iter()
                .map(|m| m.process_status_updates_ms)
                .sum();
            let total_sleep: u128 = all_metrics.iter().map(|m| m.sleep_ms).sum();
            let total_time: u128 = all_metrics.iter().map(|m| m.total_loop_ms).sum();

            let runs_pct = (total_runs as f64 / total_time as f64) * 100.0;
            let tasks_pct = (total_tasks as f64 / total_time as f64) * 100.0;
            let status_updates_pct = (total_status_updates as f64 / total_time as f64) * 100.0;
            let sleep_pct = (total_sleep as f64 / total_time as f64) * 100.0;

            format!(
                "runs:{:.1}s/{:.1}% tasks:{:.1}s/{:.1}% updates:{:.1}s/{:.1}% sleep:{:.1}s/{:.1}% total:{:.1}s ({} loops)",
                total_runs as f64 / 1000.0,
                runs_pct,
                total_tasks as f64 / 1000.0,
                tasks_pct,
                total_status_updates as f64 / 1000.0,
                status_updates_pct,
                total_sleep as f64 / 1000.0,
                sleep_pct,
                total_time as f64 / 1000.0,
                all_metrics.len()
            )
        } else {
            "waiting for metrics...".to_string()
        };

        println!(
            "[{:.1}s] Completed:{}/{} | Pending:{} Dispatched:{} Running:{} | Scheduler: {} KB, {:.1}% CPU | {}",
            elapsed,
            completed,
            total_tasks,
            pending,
            dispatched,
            running,
            resource_stats.scheduler_rss_kb,
            resource_stats.scheduler_cpu_percent,
            metrics_display
        );
    }

    let end_time = Instant::now();
    let total_duration = end_time.duration_since(start_time).as_secs_f64();

    println!();
    println!();
    println!("=== Performance Test Results ===");

    // Analyze scheduler metrics
    if !all_metrics.is_empty() {
        let total_runs_ms: u128 = all_metrics.iter().map(|m| m.process_pending_runs_ms).sum();
        let total_tasks_ms: u128 = all_metrics.iter().map(|m| m.process_pending_tasks_ms).sum();
        let total_status_updates_ms: u128 = all_metrics
            .iter()
            .map(|m| m.process_status_updates_ms)
            .sum();
        let total_sleep_ms: u128 = all_metrics.iter().map(|m| m.sleep_ms).sum();
        let total_loop_ms: u128 = all_metrics.iter().map(|m| m.total_loop_ms).sum();

        println!();
        println!("Scheduler Time Breakdown:");
        println!("  Total scheduler loops: {}", all_metrics.len());
        println!(
            "  Time processing runs: {:.2}s ({:.1}%)",
            total_runs_ms as f64 / 1000.0,
            (total_runs_ms as f64 / total_loop_ms as f64) * 100.0
        );
        println!(
            "  Time processing tasks: {:.2}s ({:.1}%)",
            total_tasks_ms as f64 / 1000.0,
            (total_tasks_ms as f64 / total_loop_ms as f64) * 100.0
        );
        println!(
            "  Time processing status updates: {:.2}s ({:.1}%)",
            total_status_updates_ms as f64 / 1000.0,
            (total_status_updates_ms as f64 / total_loop_ms as f64) * 100.0
        );
        println!(
            "  Time sleeping: {:.2}s ({:.1}%)",
            total_sleep_ms as f64 / 1000.0,
            (total_sleep_ms as f64 / total_loop_ms as f64) * 100.0
        );
        println!(
            "  Total scheduler time: {:.2}s",
            total_loop_ms as f64 / 1000.0
        );
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

    if let (tasks, Some(avg), Some(min), Some(max)) = latency_stats {
        println!();
        println!("Latency Stats:");
        println!("  Tasks: {}", tasks);
        println!("  Avg latency: {:.3}s", avg);
        println!("  Min latency: {:.3}s", min);
        println!("  Max latency: {:.3}s", max);
    }

    // Calculate throughput
    let task_throughput = total_tasks as f64 / total_duration;
    let run_submission_rate = config.workflows as f64 / trigger_duration;

    println!();
    println!("Throughput:");
    println!("  Run submission: {:.2} runs/sec", run_submission_rate);
    println!("  Task completion: {:.2} tasks/sec", task_throughput);
    println!("  Total duration: {:.2}s", total_duration);
    println!();
    println!("Explanation:");
    println!(
        "  - {} runs created ({} tasks each = {} total tasks)",
        config.workflows, config.tasks_per_workflow, total_tasks
    );
    println!("  - Run submission measures how fast we enqueue work");
    println!("  - Task completion measures actual work throughput");

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
    scheduler.kill()?;
    let _ = std::fs::remove_file(&scheduler_config_path);
    let _ = std::fs::remove_file(&scheduler_log_path);

    Ok(())
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
