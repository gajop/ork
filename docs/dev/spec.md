# Technical Specification

## State Machines

### Run Lifecycle

```
[*] --> pending: User triggers workflow
pending --> running: Scheduler creates tasks
running --> success: All tasks succeeded
running --> failed: Any task failed
running --> cancelled: User cancelled (future)
success --> [*]
failed --> [*]
cancelled --> [*]
```

**State transitions:**
- `pending` → `running`: Scheduler processes pending run, creates tasks, updates status
- `running` → `success`: All tasks complete with status=success
- `running` → `failed`: Any task has status=failed

### Task Lifecycle

```
[*] --> pending: Run enters running state
pending --> dispatched: Scheduler dispatches to executor
dispatched --> running: Executor reports task started
running --> success: Task completes successfully
running --> failed: Task exits non-zero or errors
success --> [*]
failed --> [*]
```

**State transitions:**
- `pending` → `dispatched`: Scheduler calls executor.execute(), updates DB
- `dispatched` → `running`: Executor sends StatusUpdate{status: "running"} via channel
- `running` → `success`/`failed`: Executor sends StatusUpdate{status: "success"/"failed"} via channel

**No retries:** Tasks fail permanently (no retry logic implemented yet)

## Scheduler Algorithm

### Event-Driven Main Loop

```rust
async fn run(&self) -> Result<()> {
    let mut status_rx = self.status_rx.lock().await;
    let mut poll_interval = interval(Duration::from_secs_f64(config.poll_interval_secs));

    loop {
        // Process pending runs
        let runs = db.get_pending_runs().await?;
        for run in runs {
            db.batch_create_tasks(run.id, task_count).await?;
            db.update_run_status(run.id, "running").await?;
        }

        // Process pending tasks (batch)
        let tasks = db.get_pending_tasks_with_workflow(max_batch).await?;
        let results = stream::iter(tasks)
            .map(|task| executor.execute(task.task_id, task.job_name, task.params))
            .buffer_unordered(max_concurrent)
            .collect().await;
        db.batch_update_task_status(&results).await?;

        // Process status updates from executors (event-driven)
        let mut updates = Vec::new();
        while let Ok(update) = status_rx.try_recv() {
            updates.push(update);
        }
        if !updates.is_empty() {
            db.batch_update_task_status(&updates).await?;
            // Check if any runs completed
            for run_id in affected_runs {
                check_run_completion(run_id).await?;
            }
        }

        // Wait for next poll or status update
        let had_work = runs > 0 || tasks > 0;
        if !had_work {
            tokio::select! {
                _ = poll_interval.tick() => {},
                Some(update) = status_rx.recv() => {
                    process_status_updates(vec![update]).await?;
                }
            }
        }
    }
}
```

### Key Optimizations

**Single JOIN Query:**
```sql
-- Get pending tasks with workflow info in one query
SELECT
    t.id as task_id,
    t.run_id,
    t.task_index,
    t.status as task_status,
    w.id as workflow_id,
    w.job_name,
    w.params
FROM tasks t
INNER JOIN runs r ON t.run_id = r.id
INNER JOIN workflows w ON r.workflow_id = w.id
WHERE t.status = 'pending'
LIMIT ?
```

**Batch Updates:**
```rust
// Update multiple tasks in single transaction
async fn batch_update_task_status(
    &self,
    updates: &[(Uuid, &str, Option<&str>, Option<&str>)]
) -> Result<()>
```

**Bounded Concurrency:**
```rust
// Limit concurrent dispatches to avoid memory explosion
stream::iter(tasks)
    .map(|task| executor.execute(...))
    .buffer_unordered(max_concurrent)
    .collect().await
```

## Executor Protocols

### Process Executor

**Dispatch:**
```rust
async fn execute(&self, task_id: Uuid, job_name: &str, params: Option<Value>) -> Result<String> {
    let child = Command::new(&script_path)
        .spawn()?;

    let pid = child.id().unwrap().to_string();

    // Monitor in background
    tokio::spawn(async move {
        let status = child.wait().await;
        status_tx.send(StatusUpdate {
            task_id,
            status: if status.success() { "success" } else { "failed" }
        });
    });

    Ok(pid)
}
```

**Status Updates:**
- Immediate: Send "running" when process starts
- On completion: Send "success" or "failed" based on exit code
- Via channel: No polling required

### Cloud Run Executor

**Dispatch:**
```rust
async fn execute(&self, task_id: Uuid, job_name: &str, params: Option<Value>) -> Result<String> {
    let auth_token = get_auth_token().await?;
    let response = http_client.post(cloud_run_url)
        .json(&CreateExecutionRequest { ... })
        .send().await?;

    let execution_name = response.json().execution_name;

    // Poll status in background
    tokio::spawn(async move {
        loop {
            let status = check_execution_status(execution_name).await;
            if status.is_terminal() {
                status_tx.send(StatusUpdate { task_id, status });
                break;
            }
            sleep(Duration::from_secs(2)).await;
        }
    });

    Ok(execution_name)
}
```

**Status Updates:**
- Periodic polling (every 2s)
- Sends update when terminal state reached
- Via channel: No database polling by scheduler

## Concurrency Model

### Thread Safety

- **Database pool**: Shared via `Arc<PostgresDatabase>`
- **Executor manager**: Shared via `Arc<ExecutorManager>`, internally uses `Mutex<HashMap>`
- **Status channel**: `mpsc::unbounded_channel` for executor → scheduler updates
- **Scheduler**: Single instance, no distributed coordination

### Async Runtime

- **Tokio**: All I/O is non-blocking
- **Bounded concurrency**: `buffer_unordered(N)` limits parallel operations
- **Background tasks**: Executors spawn monitoring tasks that send updates via channels

### No Leasing

Current implementation has no distributed scheduler support:
- Single scheduler instance assumed
- No lease acquisition/renewal
- No failover or HA

## Performance Characteristics

### Latency Breakdown

```
Task created (pending) → Task dispatched → Task running → Task complete
                0-5s              <100ms           task_duration
```

- **Pending → Dispatched**: Bounded by poll_interval (default 5s)
- **Dispatched → Running**: Executor startup (<100ms for processes)
- **Running → Complete**: Task execution time + channel latency (<1ms)

### Throughput Limits

**Database bottleneck:**
- Postgres connection pool: 10 connections
- Batch size: 100 tasks/poll
- Theoretical max: ~2000 tasks/sec

**Executor bottleneck:**
- Process executor: Limited by OS process limits (~1000s)
- Cloud Run executor: GCP API rate limits (~100 req/sec)

**Measured performance:**
- 155 tasks/sec sustained throughput
- <700ms p99 latency (create → complete)
- ~10MB RSS for 500 concurrent tasks

## Current Limitations

1. **No retries**: Tasks fail permanently
2. **No timeouts**: Tasks can run indefinitely
3. **No dependencies**: No DAG support, tasks run independently
4. **No scheduling**: Manual trigger only
5. **No distributed scheduler**: Single instance only
6. **No heartbeat monitoring**: Can't detect hung tasks
7. **No resource limits**: Executors can spawn unlimited workers
