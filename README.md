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
    timeout: 600

  transform:
    executor: python
    file: tasks/transform.py
    depends_on: [extract]

  load:
    executor: python
    file: tasks/load.py
    depends_on: [transform]
```

### 2. Write tasks

Python (with Pydantic):

```python
# tasks/extract.py
from ork import task
from pydantic import BaseModel
from src.extract import fetch_users, User

class Input(BaseModel):
    api_url: str = "https://api.example.com"

class Output(BaseModel):
    users: list[User]

@task
def main(input: Input) -> Output:
    users = fetch_users(input.api_url)
    return Output(users=users)
```

Rust (with serde):

```rust
// tasks/extract.rs
use ork::task;
use serde::{Deserialize, Serialize};
use crate::extract::{fetch_users, User};

#[derive(Deserialize)]
struct Input {
    #[serde(default = "default_api_url")]
    api_url: String,
}

fn default_api_url() -> String {
    "https://api.example.com".into()
}

#[derive(Serialize)]
struct Output {
    users: Vec<User>,
}

#[task]
fn main(input: Input) -> ork::Result<Output> {
    let users = fetch_users(&input.api_url)?;
    Ok(Output { users })
}
```

### 3. Run locally (DB-backed scheduler + web UI)

```bash
# Terminal 1: boot everything (Postgres + scheduler + web UI)
just up

# Terminal 2: create + trigger the example workflow
just example-run simple
```

Open the web UI at `http://127.0.0.1:4000` to see runs and task status.

Prefer SQLite? Use `just up-sqlite` and `just example-run-sqlite simple` instead of Docker.

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
