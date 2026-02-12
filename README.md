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

types:
  ExtractResult:
    raw_path: str
    user_count: int
  TransformResult:
    active_path: str
    active_count: int

tasks:
  extract:
    executor: python
    file: tasks/etl.py
    function: extract
    input_type:
      api_url: str
      raw_path: str
    output_type: types.ExtractResult
    inputs:
      api_url:
        const: "https://api.example.com/users"
      raw_path:
        const: "/tmp/users_raw.json"
    timeout: 600

  transform:
    executor: python
    file: tasks/etl.py
    function: transform
    depends_on: [extract]
    input_type:
      raw_path: str
      active_path: str
    output_type: types.TransformResult
    inputs:
      raw_path:
        ref: tasks.extract.output.raw_path
      active_path:
        const: "/tmp/users_active.json"

  load:
    executor: python
    file: tasks/etl.py
    function: load
    depends_on: [transform]
    input_type:
      active_path: str
      output_path: str
    output_type:
      output_path: str
    inputs:
      active_path:
        ref: tasks.transform.output.active_path
      output_path:
        const: "/data/active_users.json"
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

def extract(api_url: str, raw_path: str) -> dict[str, object]:
    response = requests.get(api_url, timeout=30)
    response.raise_for_status()
    users: list[User] = response.json()
    raw_file = Path(raw_path)
    raw_file.parent.mkdir(parents=True, exist_ok=True)
    raw_file.write_text(json.dumps(users), encoding="utf-8")
    return {"raw_path": raw_path, "user_count": len(users)}

def transform(raw_path: str, active_path: str) -> dict[str, object]:
    users: list[User] = json.loads(Path(raw_path).read_text(encoding="utf-8"))
    active_users = [user for user in users if user["status"] == "active"]
    active_file = Path(active_path)
    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(json.dumps(active_users), encoding="utf-8")
    return {"active_path": active_path, "active_count": len(active_users)}

def load(active_path: str, output_path: str) -> dict[str, str]:
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(
        Path(active_path).read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    return {"output_path": output_path}
```

Ork calls your functions directly. Prefer strongly typed, small inputs/outputs (paths, IDs, counts), and avoid passing large payloads between tasks.

### 3. Install and run

```bash
# Install the Ork CLI (requires Rust toolchain)
cargo install --path .

# Run the workflow end-to-end (defaults to sqlite://./.ork/ork.db?mode=rwc)
ork run workflows/etl.yaml
```

**First time?** Start the server first to see the UI:

```bash
# terminal 1 - start the scheduler and UI server
ork serve

# terminal 2 - run your workflow
ork run workflows/etl.yaml
```

Then open [http://127.0.0.1:4000](http://127.0.0.1:4000) to see the workflow execution in the UI.

### 3b. Python package status

Target package UX for Python users (future):

```bash
uv add -d ork
ork run workflows/etl.yaml
```

This is not available yet: `ork` is not published to PyPI today.
For now, install from source using `cargo install --path .` (requires Rust toolchain).

### 3c. Start the UI and open it in your browser

`ork run workflows/etl.yaml` executes a workflow, but does not start the web UI.

To run the scheduler + UI locally:

```bash
# terminal 1 - start the scheduler and UI server
ork serve
```

Then open:

```text
http://127.0.0.1:4000
```

You can keep terminal 1 running, and in terminal 2 run workflows with `ork run workflows/etl.yaml`.

Note: `ork serve` starts the scheduler with the web UI. The database defaults to `sqlite://./.ork/ork.db?mode=rwc`.

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
