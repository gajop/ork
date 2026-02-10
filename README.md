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
    file: tasks/etl.py
    function: extract
    input:
      api_url: "https://api.example.com/users"
      raw_path: "/tmp/users_raw.json"
    timeout: 600

  transform:
    executor: python
    file: tasks/etl.py
    function: transform
    input:
      raw_path: "/tmp/users_raw.json"
      active_path: "/tmp/users_active.json"
    depends_on: [extract]

  load:
    executor: python
    file: tasks/etl.py
    function: load
    input:
      active_path: "/tmp/users_active.json"
      output_path: "/data/active_users.json"
    depends_on: [transform]
```

### 2. Write tasks

Just write plain functions — no decorators, no SDK imports, no framework code:

```python
# tasks/etl.py
import json
from pathlib import Path
from typing import TypedDict

import requests

class User(TypedDict):
    id: int
    status: str

def extract(api_url: str, raw_path: str) -> int:
    response = requests.get(api_url, timeout=30)
    response.raise_for_status()
    users: list[User] = response.json()
    raw_file = Path(raw_path)
    raw_file.parent.mkdir(parents=True, exist_ok=True)
    raw_file.write_text(json.dumps(users), encoding="utf-8")
    return len(users)

def transform(raw_path: str, active_path: str) -> int:
    users: list[User] = json.loads(Path(raw_path).read_text(encoding="utf-8"))
    active_users = [user for user in users if user["status"] == "active"]
    active_file = Path(active_path)
    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(json.dumps(active_users), encoding="utf-8")
    return len(active_users)

def load(active_path: str, output_path: str) -> str:
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(
        Path(active_path).read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    return output_path
```

Ork calls your functions directly. Prefer strongly typed, small inputs/outputs (paths, IDs, counts), and avoid passing large payloads between tasks.

### 3. Run the workflow locally

```bash
# One-time local install of the Ork CLI binary from this repo
just install

# Run the workflow end-to-end (defaults to sqlite://./.ork/ork.db?mode=rwc)
ork run workflows/etl.yaml
```

Target package UX for Python users is:

```bash
uv add -d ork
ork run workflows/etl.yaml
```

(`uv add -d ork` is not available yet because the package is not published.)

For contributor-focused DB-backed scheduler + web UI setup, see [Running Locally](docs/dev/running.md).

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
