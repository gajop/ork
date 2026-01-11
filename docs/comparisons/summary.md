# workflow definition comparison - summary

## overview

**Findings:** Ork requires 70 lines across 4 files for a 3-task workflow. This is 2-3x more verbose than all competitors.

**Key observation:** Functional `@task`/`@workflow` decorators could reduce to 25-30 lines in 1 file.

**Trade-off:** Would make Ork competitive with Prefect/Airflow while maintaining serverless advantage.

## The Verbosity Problem (By the Numbers)

```
Same 3-task workflow:

Ork Current:      70 lines, 4 files  ████████████████████
Temporal:         80 lines, 1 file   ████████████████████████
Dagster:          55 lines, 1 file   ██████████████
Airflow:          45 lines, 1 file   ███████████
Kestra:           45 lines, 1 file   ███████████
Prefect:          35 lines, 1 file   █████████
Mage:             35 lines, 1 file   █████████
Ork Proposed:     25 lines, 1 file   ██████  ← BEST
```

## Current vs Proposed

### Current Ork (4 files, 70 lines)

```python
# tasks/hello.py (20 lines)
from pydantic import BaseModel

class HelloInput(BaseModel):
    name: str = "world"

class HelloOutput(BaseModel):
    message: str

def main(input: HelloInput) -> HelloOutput:
    message = f"Hello, {input.name}!"
    return HelloOutput(message=message)

# tasks/compliment.py (25 lines)
class GreetOutput(BaseModel):  # DUPLICATE!
    message: str

class ComplimentInput(BaseModel):
    greet: GreetOutput
    adjective: str = "fast"

class ComplimentOutput(BaseModel):
    line: str

def main(input: ComplimentInput) -> ComplimentOutput:
    line = f"{input.greet.message} — {input.adjective}!"
    return ComplimentOutput(line=line)

# demo.yaml (15 lines)
tasks:
  greet:
    executor: python
    file: tasks/hello.py
  compliment:
    executor: python
    file: tasks/compliment.py
    depends_on: [greet]

# TOTAL: 4 files, 70 lines
```

### Proposed Ork (1 file, 25 lines)

```python
# demo.py
from ork import task, workflow

@task
def greet(name: str = "world") -> dict:
    message = f"Hello, {name}!"
    return {"message": message}

@task
def compliment(greet: dict, adjective: str = "fast") -> dict:
    line = f"{greet['message']} — {adjective}!"
    return {"line": line}

@workflow
def demo():
    g = greet(name="Ork dev")
    compliment(g, adjective="ridiculously fast")

if __name__ == "__main__":
    demo.run()

# TOTAL: 1 file, 25 lines
```

## Critical Findings from Tool Comparison

Analyzed 8 competing tools. Full analysis in `analysis.md`.

### 1. Everyone Uses Functional Decorators

**All modern tools:**
- Airflow (43.8k ⭐): `@task` / `@dag`
- Kestra (26.2k ⭐): YAML-first, but growing fastest!
- Prefect (21.3k ⭐): `@task` / `@flow`
- Temporal (17.5k ⭐): `@activity` / `@workflow`
- Dagster (14.7k ⭐): `@asset` / `@op`
- Mage (8.6k ⭐): `@data_loader` / `@transformer`

**Ork is the ONLY tool where `@task` is non-functional.**

### 2. Schema Duplication is Unique to Ork

No other tool requires redefining upstream outputs:

```python
# Current Ork - ONLY tool that does this
class Task1Output(BaseModel):
    message: str

class Task2Input(BaseModel):
    task1: Task1Output  # Must redefine!

# Every other tool
@task
def task2(task1_output: dict):  # Just use it
    ...
```

### 3. Ork's Serverless Advantage is Being Wasted

**Server requirements:**
- ❌ Requires server: Airflow, Dagster, Temporal, Mage, Kestra
- ✅ No server: **Ork**, Prefect (optional), GCP Workflows (managed)

**Ork's serverless + local development is a MAJOR differentiator.**

We're killing it with bad ergonomics.

### 4. Two Viable Patterns

**Python-First** (Prefect, Mage) → 35 lines
**YAML-First** (Kestra, GCP) → 45-60 lines

**Ork should excel at BOTH.**

## Possible Approaches

### Functional @task Decorator (Python-First)

**Concept:** Enable pure Python workflows without YAML

**How it works:**

```python
@task
def greet(name: str = "world") -> dict:
    return {"message": f"Hello, {name}!"}
```

**Behind the scenes:**
1. `@task` introspects function signature
2. Auto-generates Pydantic Input model from params
3. Auto-generates Output model from return type
4. Validates at runtime
5. Passes to downstream tasks

**Benefits:**
- No manual Input/Output classes (saves 30+ lines per task)
- No schema duplication
- Type hints provide IDE support
- Can opt-in to Pydantic for strict validation
- Backwards compatible (YAML still works)

### Enhanced YAML (Inline Scripts)

**Concept:** Reduce file count for simple YAML workflows

**Current:**
```yaml
greet:
  executor: python
  file: tasks/hello.py  # Separate file required
```

**Proposed:**
```yaml
greet:
  executor: python
  script: |
    message = f"Hello, {inputs.name}!"
    return {"message": message}
  inputs:
    name: "Ork dev"
```

**Benefits:**
- No separate task file for simple logic
- Still supports `file:` for complex tasks
- Similar to Kestra pattern (26.2k ⭐)

### Built-in Task Types

**Concept:** Common operations without code

```yaml
tasks:
  fetch_api:
    type: http
    url: "https://api.example.com/data"
    method: GET

  load_db:
    type: sql
    query: "INSERT INTO ..."

  run_dbt:
    type: dbt
    models: [staging.*]
```

**Possible types:**
- `http`: API calls
- `sql`: Database queries
- `dbt`: dbt model runs
- `slack`: Notifications
- `gcs`/`s3`: Cloud storage

## Metrics

**Current state:**
- 70 lines, 4 files for 3-task workflow
- @task decorator is decorative
- Schema duplication in every workflow
- High barrier to entry

**With proposed changes:**
- 25 lines, 1 file for 3-task workflow
- @task decorator is functional
- No schema duplication
- Competitive with Prefect/Airflow
- Maintains serverless advantage

## References

- Tool examples: This directory (8 tools)
- Detailed analysis: `analysis.md`
- Official docs: `authoritative_examples.md`
