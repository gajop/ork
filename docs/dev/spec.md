Status: Pending Review

# Technical Specification

## State Machines

### Run Lifecycle

```
[*] --> pending: created
pending --> running: scheduler picks up
running --> success: all tasks succeeded
running --> failed: any task failed (no retries left)
running --> cancelled: user cancelled
success --> [*]
failed --> [*]
cancelled --> [*]
```

### Task Lifecycle

```
[*] --> pending: run created
pending --> dispatched: scheduler writes spec + creates job
dispatched --> running: worker starts, writes status
running --> success: exit 0
running --> failed: exit non-zero / timeout / heartbeat stale
failed --> pending: retry (if attempts < max)
failed --> [*]: no retries left
pending --> skipped: upstream failed
success --> [*]
skipped --> [*]
```

## Scheduler Algorithm

### Main Loop

```
scheduler_main():
    while true:
        acquire_lease_or_exit()
        
        check_cron_schedules()      # every 60s
        dispatch_pending_tasks()    # every 5s
        monitor_running_tasks()     # every 10s
        
        sleep(1s)
```

### Cron Check

```
check_cron_schedules():
    schedules = query schedules where next_run <= now() and enabled = true
    
    for schedule in schedules:
        create_run(schedule.workflow_id)
        schedule.next_run = compute_next(schedule.cron_expr)
        update schedule
```

### Task Dispatch

```
dispatch_pending_tasks():
    for run in query runs where status = 'running':
        # Find tasks ready to run (deps satisfied)
        ready_tasks = find_ready_tasks(run)
        
        for task in ready_tasks:
            # Atomic acquire
            acquired = try_acquire_task(task)
            
            if acquired:
                write_spec_to_object_store(task)
                create_worker_job(task)
                task.status = 'dispatched'
                update task

find_ready_tasks(run):
    pending = query tasks where run_id = run.id and status = 'pending'
    
    ready = []
    for task in pending:
        deps_satisfied = all upstream tasks have status = 'success'
        if deps_satisfied:
            ready.append(task)
    
    return ready
```

### Task Monitoring

```
monitor_running_tasks():
    tasks = query tasks where status in ('dispatched', 'running')
    
    for task in tasks:
        status = read_status_from_object_store(task)
        
        if status is None and task.status == 'dispatched':
            if now() - task.dispatched_at > 5min:
                mark_failed(task, "job never started")
            continue
        
        if status.status == 'running':
            if now() - status.heartbeat > 60s:
                mark_failed(task, "heartbeat timeout")
            elif now() - task.started_at > task.timeout:
                mark_failed(task, "execution timeout")
        
        elif status.status == 'success':
            task.output = read_output_from_object_store(task)
            task.status = 'success'
            update task
            check_run_completion(task.run_id)
        
        elif status.status == 'failed':
            if task.attempt < task.max_retries:
                task.status = 'pending'
                update task
            else:
                mark_failed(task, status.error)
                skip_downstream(task)

mark_failed(task, error):
    task.status = 'failed'
    task.error = error
    update task
    check_run_completion(task.run_id)

skip_downstream(task):
    downstream = find_downstream_tasks(task)
    for t in downstream:
        if t.on_failure != 'run':
            t.status = 'skipped'
            update t
```

### Run Completion

```
check_run_completion(run_id):
    tasks = query tasks where run_id = run_id
    
    active = count where status in ('pending', 'dispatched', 'running')
    failed = count where status = 'failed'
    
    if active == 0:
        run = get run
        if failed > 0:
            run.status = 'failed'
        else:
            run.status = 'success'
        run.finished_at = now()
        update run
```

## Worker Algorithm

```
worker_main():
    spec = fetch_from_object_store(env.TASK_SPEC_PATH)
    
    write_status("running")
    start_heartbeat_thread()
    
    ctx = Context(
        task_id=spec.task_id,
        run_id=spec.run_id,
        task_name=spec.task_name,
        workflow_name=spec.workflow_name,
        attempt=spec.attempt,
        parallel_index=spec.parallel_index,
        parallel_count=spec.parallel_count,
    )
    
    result = execute_task(spec, ctx)
    
    if result.success:
        write_output(result.output)
        write_status("success")
        exit(0)
    else:
        write_status("failed", error=result.error)
        exit(1)

heartbeat_thread():
    while running:
        write_status("running")
        sleep(10s)

execute_task(spec, ctx):
    executor = load_executor(spec.executor)
    input = merge(spec.input, spec.upstream)
    return executor.run(input, ctx)
```

## Distributed Coordination

### Lease Acquisition

Prevents multiple scheduler instances from processing the same run.

```
acquire_lease(run_id, scheduler_id):
    # Atomic compare-and-swap
    result = update runs
        set owner = scheduler_id, lease_expires = now() + 60s
        where id = run_id
        and status = 'running'
        and (owner is null or lease_expires < now())
    
    return result.modified_count > 0

renew_lease(run_id, scheduler_id):
    update runs
        set lease_expires = now() + 60s
        where id = run_id and owner = scheduler_id

release_lease(run_id, scheduler_id):
    update runs
        set owner = null, lease_expires = null
        where id = run_id and owner = scheduler_id
```

### Task Acquisition

Prevents multiple dispatches of the same task.

For Postgres:

```sql
UPDATE task_runs
SET status = 'dispatched',
    dispatched_at = now(),
    attempt = attempt + 1
WHERE id = (
    SELECT id FROM task_runs
    WHERE run_id = $run_id AND status = 'pending'
    LIMIT 1
    FOR UPDATE SKIP LOCKED
)
RETURNING *;
```

For document stores, use conditional updates or transactions.

## Parallelization

### Static Parallelization

```yaml
tasks:
  process:
    parallel: 4
```

Creates 4 task runs with:
- `parallel_index`: 0, 1, 2, 3
- `parallel_count`: 4

### Dynamic by Count

```yaml
tasks:
  discover:
    # outputs: { "count": 10 }
  process:
    depends_on: [discover]
    parallel: discover.count
```

After `discover` completes, reads `count` from output and creates 10 task runs.

### Dynamic by Items

```yaml
tasks:
  discover:
    # outputs: { "files": ["a.csv", "b.csv", "c.csv"] }
  process:
    depends_on: [discover]
    foreach: discover.files
```

After `discover` completes, reads `files` from output and creates 3 task runs, each with:
- `input.item`: the file path
- `parallel_index`: 0, 1, 2
- `parallel_count`: 3

### Concurrency Limit

```yaml
tasks:
  process:
    foreach: discover.files
    concurrency: 4
```

Scheduler dispatches at most 4 `process` tasks at a time. Others remain `pending` until a slot opens.

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | /workflows | List workflows |
| POST | /workflows | Create/update workflow |
| GET | /workflows/{name} | Get workflow |
| DELETE | /workflows/{name} | Delete workflow |
| POST | /workflows/{name}/runs | Trigger run |
| GET | /runs | List runs |
| GET | /runs/{id} | Get run details |
| POST | /runs/{id}/cancel | Cancel run |
| POST | /runs/{id}/retry | Retry failed tasks |
| GET | /runs/{id}/tasks | List task runs |
| GET | /runs/{id}/tasks/{name}/logs | Get task logs |
