Status: Pending Review

# Failure Handling

## Default Behavior

When a task fails:
1. Task marked as `failed`
2. Downstream tasks marked as `skipped`
3. Run marked as `failed` when all tasks complete
4. Scheduler stops dispatching new tasks for the run

## Retry on Failure

```yaml
tasks:
  flaky_api_call:
    executor: python
    file: tasks/api.py
    retries: 3
    retry_delay: 60
```

See [Configuration - Retries](configuration.md#retries) for details.

## Run Despite Failure

Some tasks should run even when upstream tasks fail (e.g., cleanup, notifications):

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py

  transform:
    executor: python
    file: tasks/transform.py
    depends_on: [extract]

  load:
    executor: python
    file: tasks/load.py
    depends_on: [transform]

  # Notification runs regardless of success/failure
  notify:
    executor: python
    file: tasks/notify.py
    depends_on: [load]
    on_failure: run  # Run even if load fails

  # Cleanup always runs
  cleanup:
    executor: python
    file: tasks/cleanup.py
    depends_on: [extract, transform, load]
    on_failure: run
```

### `on_failure` Options

- `skip` (default): Skip if any dependency failed
- `run`: Run even if dependencies failed

## Detecting Failures in Tasks

Tasks with `on_failure: run` can check which dependencies succeeded:

```py
# tasks/notify.py
from ork import task, Context
from pydantic import BaseModel

class ExtractOutput(BaseModel):
    status: str

class TransformOutput(BaseModel):
    status: str

class LoadOutput(BaseModel):
    status: str

class Input(BaseModel):
    extract: ExtractOutput | None = None
    transform: TransformOutput | None = None
    load: LoadOutput | None = None

class Output(BaseModel):
    notified: bool

@task
def main(input: Input, ctx: Context) -> Output:
    if input.load is None:
        # Load failed, send failure notification
        send_alert(f"Workflow {ctx.workflow_name} failed")
    else:
        # Success, send success notification
        send_notification(f"Workflow {ctx.workflow_name} completed")

    return Output(notified=True)
```

When upstream tasks fail, their outputs are `None`.

## Partial Retries

Retry only failed tasks without re-running successful ones:

```bash
ork run retry <run_id>
```

Or retry a specific task:

```bash
ork run retry <run_id> --task transform
```

## Cancellation

Cancel a running workflow:

```bash
ork run cancel <run_id>
```

Behavior:
1. Run marked as `cancelled`
2. Running tasks continue to completion
3. Pending tasks marked as `skipped`
4. No new tasks dispatched

## Error Messages

Tasks can provide structured error information:

```py
@task
def main(input: Input) -> Output:
    try:
        result = risky_operation()
        return Output(result=result)
    except ValueError as e:
        raise TaskError(
            message="Invalid input data",
            details={"error": str(e), "input": input.dict()}
        )
```

Error details are stored and visible in:
- Run status
- Task logs
- API responses

## Heartbeat Failures

Tasks must send heartbeats every 10 seconds. If no heartbeat for 60 seconds:
1. Scheduler marks task as `failed`
2. Error: "heartbeat timeout"
3. Executor may kill the worker process

This detects:
- Worker crashes
- Network failures
- Hung processes

## Timeout Failures

If a task exceeds its `timeout`:
1. Scheduler marks task as `failed`
2. Error: "execution timeout"
3. Executor kills the worker process

```yaml
tasks:
  slow_task:
    executor: python
    file: tasks/slow.py
    timeout: 7200  # 2 hours
```

## Alerting

### Built-in Notifications

Configure notifications in the scheduler:

```yaml
# ork.yaml
notifications:
  on_failure:
    email:
      to: [team@example.com]
    slack:
      webhook_url:
        secret: projects/my-project/secrets/slack-webhook
```

### Custom Notification Tasks

Add notification tasks to workflows:

```yaml
tasks:
  # Main workflow
  extract: {}
  transform:
    depends_on: [extract]
  load:
    depends_on: [transform]

  # Notification
  notify_failure:
    executor: python
    file: tasks/notify.py
    depends_on: [load]
    on_failure: run
```

```py
# tasks/notify.py
@task
def main(input: Input, ctx: Context) -> Output:
    if any(output is None for output in [input.extract, input.transform, input.load]):
        send_pagerduty_alert(
            severity="error",
            message=f"Workflow {ctx.workflow_name} failed",
            run_id=ctx.run_id
        )
    return Output(notified=True)
```
