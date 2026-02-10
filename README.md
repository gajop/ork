Status: Pending Review

# Ork

Serverless workflow orchestrator with near-zero idle cost.

## Goals

1. **Low/no cost at idle** — Pay only for actual compute. Built in Rust for minimal resource usage, with architecture designed to avoid consumption when idle. First-class support for serverless backends (Cloud Run, Lambda, Azure Container Apps) that scale to zero.

2. **Parallelization included** — Orchestration and parallel compute in one. Fan out to serverless workers (Cloud Run Jobs, Fargate, K8s Jobs) without managing infrastructure. Scale from one task to thousands.

3. **Orchestration as afterthought** — Write business logic first, add orchestration later. Factory-driven, statically-defined DAGs let you focus on what your code does, not how it's scheduled. No framework lock-in in your task code.

4. **Fully open source** — No company behind this, no "enterprise" edition, no feature gating. Apache-2.0 OR MIT licensed. Community-driven.

## Quick Start

### 1. Define a workflow

```yaml
# workflows/etl.yaml
name: my_etl

tasks:
  extract:
    executor: python
    file: tasks/extract.py
    input:
      api_url: "https://api.example.com/users"
    timeout: 600

  transform:
    executor: python
    file: tasks/transform.py
    depends_on: [extract]

  load:
    executor: python
    file: tasks/load.py
    input:
      output_path: "/data/active_users.json"
    depends_on: [transform]
```

### 2. Write tasks

Just write plain functions — no decorators, no SDK imports, no framework code:

```python
# tasks/extract.py
import requests

def main(api_url: str) -> list[dict]:
    return requests.get(api_url).json()
```

```python
# tasks/transform.py
def main(records: list[dict]) -> list[dict]:
    return [r for r in records if r["status"] == "active"]
```

```python
# tasks/load.py
import json

def main(records: list[dict], output_path: str) -> int:
    with open(output_path, "w") as f:
        json.dump(records, f)
    return len(records)
```

Ork calls your functions directly — arguments come from the workflow YAML, return values flow to downstream tasks.

### 3. Run locally (DB-backed scheduler + web UI)

```bash
# Terminal 1: boot everything (Postgres + scheduler + web UI)
just up

# Terminal 2: create + trigger the example workflow
just example simple
```

Open the web UI at `http://127.0.0.1:4000` to see runs and task status.

Examples run via SQLite by default: `just example simple`.

### 4. Deploy

See [Deployment](docs/deployment.md) for deployment options.

## Documentation

**Getting Started:**
- [Quick Start](docs/quickstart.md) - Get up and running in 5 minutes
- [Concepts](docs/concepts.md) - Core terminology: workflows, tasks, runs

**Writing Workflows:**
- [Workflows](docs/workflows.md) - Defining DAGs and dependencies
- [Tasks](docs/tasks.md) - Writing task wrappers with inputs/outputs
- [Executors](docs/executors.md) - Built-in and custom executors
- [Parallelization](docs/parallelization.md) - Static, dynamic, and foreach patterns

**Advanced Topics:**
- [dbt Integration](docs/dbt.md) - Importing dbt projects
- [Failure Handling](docs/failure_handling.md) - Error handling and recovery
- [Context](docs/context.md) - Accessing execution metadata
- [Operations](docs/operations.md) - CLI commands and scheduling

**Configuration & Deployment:**
- [Configuration](docs/configuration.md) - Retries, resources, environment variables, secrets
- [Deployment](docs/deployment.md) - GCP, local development
- [Type Schemas](docs/type_schemas.md) - Type definitions and validation

**Developer Docs:**
- [Architecture](docs/dev/architecture.md) - System overview, components, data flow
- [Running Locally](docs/dev/running.md) - One-command boot + example execution
- [Schema](docs/dev/schema.md) - Database structure, object storage, JSON formats
- [Spec](docs/dev/spec.md) - Algorithms, state machines, distributed coordination
- [Crates](docs/dev/crates.md) - Rust crate structure and interfaces
- [Implementation Plan](docs/dev/implementation_plan.md) - Phased build plan

## License

Apache-2.0 OR MIT
