Status: Pending Review

# Quick Start

This guide shows how to add orchestration to existing code with a 2-task workflow.

## Your Existing Code

You have a script that fetches data and processes it:

```py
# src/data_ops.py
import requests

def fetch_data(url: str) -> str:
    """Fetch data from URL"""
    response = requests.get(url)
    response.raise_for_status()
    return response.text

def summarize_data(text: str) -> dict:
    """Compute summary statistics"""
    lines = text.split('\n')

    return {
        "total_chars": len(text),
        "total_lines": len(lines),
        "non_empty_lines": len([l for l in lines if l.strip()])
    }
```

You run it like:

```py
from src.data_ops import fetch_data, summarize_data

# Fetch
data = fetch_data("https://example.com/data.json")
print(f"Downloaded {len(data)} bytes")

# Process
stats = summarize_data(data)
print(f"Stats: {stats}")
```

**Problems**:
- Runs sequentially (no parallelization when fetching multiple sources)
- No retries on network failures
- No scheduling
- Hard to monitor which step failed

## Add Orchestration

### 1. Wrap Your Functions

Create task wrappers that define inputs and outputs:

```py
# tasks/fetch.py
from ork import task
from pydantic import BaseModel
from src.data_ops import fetch_data

class Input(BaseModel):
    url: str

class Output(BaseModel):
    data: str

@task
def main(input: Input) -> Output:
    data = fetch_data(input.url)
    return Output(data=data)
```

```py
# tasks/summarize.py
from ork import task
from pydantic import BaseModel
from src.data_ops import summarize_data

class FetchOutput(BaseModel):
    data: str

class Input(BaseModel):
    fetch: FetchOutput  # ← Receives output from fetch task

class Output(BaseModel):
    total_chars: int
    total_lines: int
    non_empty_lines: int

@task
def main(input: Input) -> Output:
    stats = summarize_data(input.fetch.data)
    return Output(**stats)
```

**Key points**:
- `Input` and `Output` define the interface
- Your existing functions (`fetch_data`, `summarize_data`) don't change
- The `summarize` task receives `fetch` output via `input.fetch`
- Data passes through task outputs, not files

### 2. Define the Workflow

```yaml
# workflows/data_pipeline.yaml
name: data_pipeline

tasks:
  fetch:
    executor: python
    file: tasks/fetch.py
    input:
      url: https://example.com/data.json
    timeout: 300
    retries: 3

  summarize:
    executor: python
    file: tasks/summarize.py
    depends_on: [fetch]
```

**What this does**:
- `fetch` runs first with retries on failure
- `summarize` runs after `fetch` succeeds
- `summarize` receives the output from `fetch` via `input.fetch`

### 3. What You Get

Ork now handles:
- **Dependencies** - `summarize` waits for `fetch` to complete
- **Retries** - Network failures retry automatically
- **Scheduling** - Add `schedule: "0 * * * *"` to run hourly
- **Monitoring** - See which task failed and view logs
- **Parallelization** - Scale to many URLs by adding more fetch tasks
- **Low idle cost** - When deployed serverless, costs ~$0 when not running

**Your business logic stayed the same** - you just wrapped it with input/output definitions and told Ork how to run it.

## Design Principles

1. **Orchestration as afterthought** - Add orchestration without rewriting business logic
2. **Low/no cost at idle** - Serverless deployment scales to zero when not running
3. **Parallelization included** - Run multiple fetch tasks in parallel (see [Parallelization](parallelization.md))
4. **Fully open source** - No enterprise edition, no feature gating

## Next Steps

- [Tutorial: ELT Pipeline](tutorial_elt_pipeline.md) - Realistic example with medallion architecture
- [Concepts](concepts.md) - Core concepts and terminology
- [Workflows](workflows.md) - DAG definitions and dependencies
- [Tasks](tasks.md) - Writing task wrappers in detail
- [Deployment](deployment.md) - Deploy to GCP or run locally
