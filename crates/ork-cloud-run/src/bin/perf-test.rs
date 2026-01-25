use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use sqlx::PgPool;
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

#[derive(Debug, Deserialize)]
struct SchedulerConfig {
    poll_interval_secs: u64,
    max_tasks_per_batch: i64,
    max_concurrent_dispatches: usize,
    max_concurrent_status_checks: usize,
}

#[derive(Debug)]
struct ResourceStats {
    scheduler_rss_kb: u64,
    scheduler_cpu_percent: f32,
    worker_count: usize,
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
    println!("Total tasks: {}", config.workflows * config.tasks_per_workflow);
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

    // Start scheduler in background with config
    println!("Starting scheduler...");
    println!("  Poll interval: {}s", config.scheduler.poll_interval_secs);
    println!("  Max batch: {}", config.scheduler.max_tasks_per_batch);
    println!("  Max concurrent dispatches: {}", config.scheduler.max_concurrent_dispatches);
    println!("  Max concurrent status checks: {}", config.scheduler.max_concurrent_status_checks);

    let mut scheduler = Command::new("../../target/release/ork-cloud-run")
        .args(["run"])
        .env("POLL_INTERVAL_SECS", config.scheduler.poll_interval_secs.to_string())
        .env("MAX_TASKS_PER_BATCH", config.scheduler.max_tasks_per_batch.to_string())
        .env("MAX_CONCURRENT_DISPATCHES", config.scheduler.max_concurrent_dispatches.to_string())
        .env("MAX_CONCURRENT_STATUS_CHECKS", config.scheduler.max_concurrent_status_checks.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
    let mut max_worker_count = 0usize;

    while completed < total_tasks {
        sleep(Duration::from_millis(500)).await;

        // Get task count
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE status IN ('success', 'failed')")
                .fetch_one(&pool)
                .await?;
        completed = row.0 as u32;

        // Get resource stats
        system.refresh_processes(ProcessesToUpdate::Some(&[scheduler_pid]), true);
        let resource_stats = if let Some(process) = system.process(scheduler_pid) {
            let worker_count = get_child_process_count(scheduler_pid.as_u32());
            max_worker_count = max_worker_count.max(worker_count);

            ResourceStats {
                scheduler_rss_kb: process.memory() / 1024, // Convert bytes to KB
                scheduler_cpu_percent: process.cpu_usage(),
                worker_count,
            }
        } else {
            ResourceStats {
                scheduler_rss_kb: 0,
                scheduler_cpu_percent: 0.0,
                worker_count: 0,
            }
        };

        print!(
            "\rProgress: {}/{} tasks | Scheduler: {} KB RSS, {:.1}% CPU | Workers: {}   ",
            completed,
            total_tasks,
            resource_stats.scheduler_rss_kb,
            resource_stats.scheduler_cpu_percent,
            resource_stats.worker_count
        );
        use std::io::Write;
        std::io::stdout().flush()?;
    }

    let end_time = Instant::now();
    let total_duration = end_time.duration_since(start_time).as_secs_f64();

    println!();
    println!();
    println!("=== Performance Test Results ===");

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
    println!("  - {} runs created ({} tasks each = {} total tasks)", config.workflows, config.tasks_per_workflow, total_tasks);
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
    println!("  Peak worker count: {}", max_worker_count);

    // Cleanup
    println!();
    println!("Cleaning up...");
    scheduler.kill()?;

    Ok(())
}

fn get_child_process_count(parent_pid: u32) -> usize {
    let output = Command::new("pgrep")
        .args(["-P", &parent_pid.to_string()])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|line| !line.is_empty())
                    .count()
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}
