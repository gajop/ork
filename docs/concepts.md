Status: Pending Review

# Concepts

## Workflow

A directed acyclic graph (DAG) of tasks. Defined in YAML within a container image, discovered by Ork via code locations.

```yaml
name: my_etl
schedule: "0 2 * * *"

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
```

## Task

A unit of work within a workflow. Two types:

**Ork-aware tasks:** Use the Ork SDK, have typed inputs/outputs, receive context.

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
```

**Plain container tasks:** Any Docker image, no Ork SDK required. Useful for existing code, third-party tools, or legacy systems.

```yaml
tasks:
  run_spark:
    image: apache/spark:3.5.0
    command: ["spark-submit", "/app/job.jar"]
```

## Code Location

A registered container image that Ork parses for workflow definitions. Defined in `locations.yaml` by the infra team.

```yaml
# locations.yaml
locations:
  - name: etl
    image: us-central1-docker.pkg.dev/my-project/workflows/etl:latest
    refresh: "*/15 * * * *"
    
  - name: ml-pipelines
    image: us-central1-docker.pkg.dev/my-project/workflows/ml:latest
    refresh: "0 * * * *"
```

Ork periodically runs `ork-parse` on each location to discover workflows. This keeps the deployed images as the source of truth.

**Ork-aware locations:** Container has `ork-parse` entrypoint, defines workflows in code or YAML.

**Plain container locations:** Workflows defined externally (via API), reference plain container images.

## Worker Config

Named configuration for task execution. Defined in `workers.yaml` by the infra team. Controls service account, VPC, resources.

```yaml
# workers.yaml
workers:
  extract:
    service_account: ork-extract@project.iam.gserviceaccount.com
    vpc_connector: projects/x/locations/y/connectors/external
    cpu: 1
    memory: 2Gi

  transform:
    service_account: ork-transform@project.iam.gserviceaccount.com
    vpc_connector: projects/x/locations/y/connectors/internal
    cpu: 2
    memory: 4Gi

default: extract
```

Tasks reference worker configs by name:

```yaml
tasks:
  extract:
    worker: extract
    executor: python
    file: tasks/extract.py
```

## Run

A single execution of a workflow. Created by:
- Cron schedule firing
- Manual trigger via CLI or API
- External event (future)

States: `pending` → `running` → `success` | `failed` | `cancelled`

## Task Run

A single execution of a task within a run. May have multiple attempts (retries).

States: `pending` → `dispatched` → `running` → `success` | `failed` | `skipped`

```
pending ──→ dispatched ──→ running ──→ success
                │              │
                │              ├──→ failed ──→ pending (retry)
                │              │         └──→ [final]
                │              │
                └──────────────┴──→ skipped (upstream failed)
```

## Schedule

Cron expression defining when a workflow runs automatically.

```yaml
schedule: "0 2 * * *"  # daily at 2am
```

## Executor

Runs task code. Built-in executors handle Python and Rust. Custom executors handle domain-specific patterns (SQL, HTTP, etc.).

Executors are initialized with config and receive a context object for each task run.

## Context

Execution metadata available to tasks:

- `task_id` — unique task run identifier
- `run_id` — workflow run identifier
- `task_name` — name of the task
- `workflow_name` — name of the workflow
- `attempt` — retry attempt number (starts at 1)
- `parallel_index` — instance index for parallel tasks
- `parallel_count` — total instances for parallel tasks
- `log` — structured logger
- `metrics` — metrics recorder

## Plain Container I/O

Plain containers can exchange data via files:

```yaml
tasks:
  fetch_data:
    image: .../legacy-fetcher:v1
    command: ["python", "fetch.py"]
    output:
      from_file: /output/result.json

  process:
    image: .../processor:v1
    command: ["python", "process.py"]
    depends_on: [fetch_data]
    input:
      to_file: /input/data.json
```

Ork reads output from the specified path after the container exits, and writes input to the specified path before the container starts.

## Lease

Distributed lock preventing multiple scheduler instances from processing the same run.

## Heartbeat

Timestamp indicating a component is alive.

- Scheduler → State store: lease renewal (every 30s)
- Worker → Object store: status file update (every 10s)

Stale heartbeat (>60s) indicates failure.

## State Store

Persists workflow definitions, runs, task runs, and schedules. Currently: Firestore.

## Object Store

Stores task specs, status files, and outputs for communication between scheduler and workers. Currently: GCS.
