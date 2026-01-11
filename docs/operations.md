Status: Pending Review

# Operations

## Running Workflows

### Local Development

```bash
# Start local stack
docker compose up -d

# Deploy workflow
ork workflow deploy workflows/etl.yaml

# Start a run
ork run start my_etl

# Follow logs
ork run logs <run_id> --follow

# Check status
ork run status <run_id>
```

### With Input Parameters

```bash
ork run start my_etl --input date=2024-01-15 --input region=us-west
```

Input parameters are passed to all tasks that declare them.

## CLI Reference

### Workflows

```bash
# Deploy a workflow definition
ork workflow deploy <file>

# List all workflows
ork workflow list

# Show workflow status and recent runs
ork workflow status <name>

# Delete a workflow
ork workflow delete <name>
```

### Runs

```bash
# Start a run
ork run start <workflow> [--input key=value]

# Show run status
ork run status <run_id>

# List runs
ork run list [workflow]

# View logs
ork run logs <run_id> [--task <name>] [--follow]

# Cancel a running workflow
ork run cancel <run_id>

# Retry failed tasks
ork run retry <run_id> [--task <name>]
```

### Schema

```bash
# Export type schemas from task file
ork schema export tasks/extract.py
```

### Development

```bash
# Start local development server
ork dev
```

## Scheduling

Schedule workflows with cron expressions:

```yaml
name: my_etl
schedule: "0 2 * * *"  # Daily at 2am UTC

tasks:
  # ...
```

### Cron Syntax

```
┌───────────── minute (0-59)
│ ┌───────────── hour (0-23)
│ │ ┌───────────── day of month (1-31)
│ │ │ ┌───────────── month (1-12)
│ │ │ │ ┌───────────── day of week (0-6, Sunday=0)
│ │ │ │ │
* * * * *
```

Examples:
- `0 2 * * *` - Daily at 2am
- `0 */4 * * *` - Every 4 hours
- `0 9 * * 1` - Every Monday at 9am
- `0 0 1 * *` - First day of every month

### Disabling Schedules

```bash
# Disable scheduling for a workflow
ork workflow update my_etl --schedule-enabled=false

# Re-enable
ork workflow update my_etl --schedule-enabled=true
```

## Manual Triggers

Trigger workflows on-demand even if they have a schedule:

```bash
ork run start my_etl
```

## Monitoring

### Run Status

```bash
$ ork run status abc-123-def
Run: abc-123-def
Workflow: my_etl
Status: running
Started: 2024-01-15 10:30:00
Tasks: 3/10 completed

┌──────────────┬───────────┬─────────────────────┐
│ Task         │ Status    │ Duration            │
├──────────────┼───────────┼─────────────────────┤
│ extract      │ success   │ 45s                 │
│ transform    │ success   │ 1m 23s              │
│ load         │ running   │ 34s (ongoing)       │
│ validate     │ pending   │ -                   │
└──────────────┴───────────┴─────────────────────┘
```

### Logs

```bash
# All logs for a run
ork run logs abc-123-def

# Logs for a specific task
ork run logs abc-123-def --task extract

# Follow logs in real-time
ork run logs abc-123-def --follow
```

### List Runs

```bash
# All runs
ork run list

# Runs for a specific workflow
ork run list my_etl

# Limit results
ork run list --limit 10

# Filter by status
ork run list --status failed
```
