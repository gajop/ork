# Workflow Orchestration Tools - Comprehensive Comparison

This directory contains working examples of the same workflow implemented in different orchestration tools. The workflow is simple: greet → compliment → echo (same as `examples/demo`).

**Files:**
- `complete_framework_list.md` - Survey of 24 orchestration frameworks with GitHub stars
- `1-9_*.py`, `*.yaml` - Working examples for each tool
- `analysis.md` - Detailed comparison analysis
- `authoritative_examples.md` - Official docs and GitHub links
- `summary.md` - High-level findings and quantification

## Quick Comparison Table

| Tool | Files | Lines | Server? | Verbosity | Type Safety | Local Dev | Language |
|------|-------|-------|---------|-----------|-------------|-----------|----------|
| **Airflow** (modern) | 1 | 45 | Yes | Medium | Medium | Hard | Python |
| **Prefect** | 1 | 35 | Optional | Low | Medium | Easy | Python |
| **Dagster** | 1 | 55 | Yes | High | High | Medium | Python |
| **Temporal** | 1 | 80 | Yes | Very High | High | Hard | Python |
| **GCP Workflows** | 1 | 60 | N/A (Cloud) | Medium | Low | No | YAML |
| **Kestra** | 1 | 45 | Yes | Medium | Low | Medium | YAML+Scripts |
| **Mage** | 1 | 35 | Yes | Medium | Medium | Medium | Python |
| **Ork (current)** | 4 | 70 | **No** | **Very High** | **High** | **Easy** | YAML+Python |
| **Ork (proposed Python)** | 1 | 30 | **No** | **Very Low** | Medium | **Easy** | Python |
| **Ork (proposed YAML)** | 1 | 35 | **No** | **Low** | Low | **Easy** | YAML |
| **Ork (proposed Hybrid)** | 1 | 25 | **No** | **Very Low** | **High** | **Easy** | Python |

## Detailed Comparison

### Server Requirements

**Requires Server:**
- Airflow: Webserver + Scheduler + Database (PostgreSQL/MySQL)
- Dagster: Dagster daemon + webserver + database
- Temporal: Temporal server (heavy, requires Cassandra/PostgreSQL)
- Kestra: Kestra server (Docker/K8s)
- Mage: Mage daemon
- GCP Workflows: Google Cloud (managed)

**No Server (Truly Serverless):**
- Ork (all variants): Just the CLI binary
- Prefect: Can run without server (server optional for UI/scheduling)

### Lines of Code for 3-Task Workflow

```
Temporal:     80 lines  ████████████████████
Ork Current:  70 lines  ██████████████████
GCP Workflows: 60 lines ███████████████
Dagster:      55 lines  ██████████████
Airflow:      45 lines  ███████████
Kestra:       45 lines  ███████████
Ork YAML:     35 lines  █████████
Prefect:      35 lines  █████████
Mage:         35 lines  █████████
Ork Python:   30 lines  ████████
Ork Hybrid:   25 lines  ██████
```

### Type Safety Spectrum

```
Strict ←                                → Permissive

Dagster ─ Ork Current ─ Temporal ─ Airflow ─ Prefect ─ Mage ─ GCP Workflows ─ Kestra
   │          │            │           │         │        │          │            │
   │          │            │           │         │        │          │            └─ YAML + magic strings
   │          │            │           │         │        │          └─ YAML + templating
   │          │            │           │         │        └─ Decorators + type hints
   │          │            │           │         └─ Type hints (not enforced)
   │          │            │           └─ Type hints (not enforced)
   │          │            └─ Activities/Workflows enforce types
   │          └─ Pydantic validation on every I/O
   └─ Pydantic + Asset type checking

Ork Hybrid ─ combines strong validation with low verbosity
```

### Ecosystem Maturity

**Production Ready:**
- Airflow: 10+ years, massive ecosystem, battle-tested
- Temporal: 5+ years, proven at Uber scale
- Prefect: 5+ years, large community
- Dagster: 4+ years, growing fast
- GCP Workflows: 3+ years, Google-backed

**Growing:**
- Kestra: 2+ years, open source
- Mage: 2+ years, data engineering focused

**New:**
- Ork: Early development

### Target Use Case

| Tool | Best For |
|------|----------|
| **Airflow** | Complex ETL, need scheduling, mature ecosystem |
| **Prefect** | Python-first workflows, modern Airflow alternative |
| **Dagster** | Data assets, strong lineage, software engineering rigor |
| **Temporal** | Long-running business processes, saga patterns |
| **GCP Workflows** | Simple GCP service orchestration, serverless |
| **Kestra** | Mixed-language workflows, modern UI |
| **Mage** | Data engineering, notebook-style development |
| **Ork** | Serverless data pipelines, low idle cost, dbt orchestration |

## Key Insights from Comparison

### 1. Everyone is moving to decorators
- Airflow: `@task` / `@dag` (TaskFlow API)
- Prefect: `@task` / `@flow`
- Dagster: `@asset` / `@op` / `@job`
- Temporal: `@activity` / `@workflow`
- Mage: `@data_loader` / `@transformer` / `@data_exporter`

**Ork should adopt functional decorators too!**

### 2. Two dominant patterns

**Pattern A: Python-first with decorators** (Prefect, modern Airflow, Mage)
- Single file
- Natural Python
- ~30-40 lines for simple workflows
- Easy to test
- Good for Python-heavy workloads

**Pattern B: Declarative YAML/config** (GCP Workflows, Kestra, dbt)
- Declarative
- Language-agnostic
- Better for mixed executors
- Harder to test

**Ork is trying to be both, but current implementation is too verbose!**

### 3. Nobody else requires 4 files for 3 tasks

Current Ork structure is an outlier:
```
ork/
├── workflow.yaml
├── tasks/
│   ├── task1.py  (20 lines of boilerplate)
│   ├── task2.py  (25 lines of boilerplate)
│   └── task3.py  (20 lines of boilerplate)
└── src/
    └── logic.py
```

Compare to Prefect/Airflow/Mage:
```
workflow.py (single file, 30-40 lines total)
```

### 4. Schema duplication is unique to Ork

No other tool requires you to redefine upstream outputs:

```python
# Current Ork - must redefine!
class Task1Output(BaseModel):
    message: str

class Task2Input(BaseModel):
    task1: Task1Output  # Redefined!

# Prefect/Airflow/everyone else
@task
def task2(task1_output: dict):  # Just use it!
    ...
```

### 5. Inline scripts are common

Many tools support inline scripts to avoid file proliferation:
- GCP Workflows: YAML with inline Python/shell
- Kestra: YAML with inline scripts
- Proposed Ork YAML: Should support this too

## Recommendations for Ork

Based on this analysis, here are concrete recommendations:

### Option 1: Python-first (like Prefect)
**Best for:** Simple Python workflows, low barrier to entry

```python
from ork import task, workflow

@task
def extract(url: str) -> dict:
    return {"data": fetch(url)}

@task
def transform(extract: dict) -> dict:
    return {"clean": process(extract["data"])}

@workflow
def pipeline():
    e = extract(url="...")
    transform(e)
```

**Implementation:**
1. Make `@task` decorator functional (not just docs)
2. Auto-generate Input/Output from function signatures
3. Support optional Pydantic for strict typing
4. Keep YAML support for multi-executor workflows

### Option 2: Enhanced YAML (like Kestra)
**Best for:** Mixed-language workflows, declarative approach

```yaml
tasks:
  extract:
    executor: python
    script: |
      import requests
      return {"data": requests.get(url).json()}
    inputs:
      url: "https://..."

  transform:
    executor: python
    file: tasks/transform.py  # Still support files
    depends_on: [extract]
```

**Implementation:**
1. Support inline scripts in YAML
2. Add built-in task types (http, sql, dbt)
3. Better templating (Jinja2-style)
4. Keep file references for complex logic

### Option 3: Hybrid (RECOMMENDED)
**Best for:** Flexibility for all use cases

Support BOTH patterns:

**Simple workflows → Pure Python:**
```python
from ork import task, workflow

@task
def greet(name: str) -> dict:
    return {"message": f"Hello, {name}!"}

@workflow
def demo():
    greet(name="Ork")
```

**Complex workflows → Enhanced YAML:**
```yaml
tasks:
  fetch:
    type: http
    url: "..."

  transform:
    executor: python
    file: tasks/transform.py
```

**Implementation priorities:**
1. ✅ **P0: Make @task functional** (auto-generate schemas from type hints)
2. ✅ **P0: Support pure Python workflows** (no YAML needed for simple cases)
3. **P1: Support inline scripts in YAML** (reduce file count)
4. **P2: Add built-in task types** (http, sql, dbt, slack)
5. **P3: Better templating in YAML** (Jinja2-style)

### Breaking it down: P0 Implementation

**Goal:** Reduce 70 lines / 4 files → 30 lines / 1 file

**Before:**
```python
# tasks/hello.py (20 lines)
class HelloInput(BaseModel):
    name: str

class HelloOutput(BaseModel):
    message: str

def main(input: HelloInput) -> HelloOutput:
    return HelloOutput(message=f"Hello, {input.name}")

# workflow.yaml
tasks:
  greet:
    executor: python
    file: tasks/hello.py
```

**After:**
```python
# demo.py (single file)
from ork import task, workflow

@task
def greet(name: str) -> dict:
    return {"message": f"Hello, {name}!"}

@workflow
def demo():
    greet(name="Ork")

if __name__ == "__main__":
    demo.run()
```

**How @task works:**

1. Introspect function signature: `greet(name: str) -> dict`
2. Auto-generate: `GreetInput(BaseModel)` with `name: str` field
3. Auto-generate: `GreetOutput` from return type
4. Validate at runtime
5. Store in workflow DAG

**Backwards compatible:** Keep YAML + manual Pydantic for those who want it

## Files in This Directory

1. `1_airflow_demo.py` - Modern Airflow with @task/@dag decorators
2. `2_prefect_demo.py` - Prefect 2.0 with @task/@flow
3. `3_dagster_demo.py` - Dagster with assets and ops
4. `4_temporal_demo.py` - Temporal with activities/workflows
5. `5_gcp_workflows_demo.yaml` - Google Cloud Workflows
6. `6_kestra_demo.yaml` - Kestra with inline scripts
7. `7_mage_demo.py` - Mage with decorators
8. `8_ork_current.py` - Current Ork (documented, not runnable)
9. `9_ork_proposed_python.py` - Proposed pure Python approach
10. `10_ork_proposed_yaml.yaml` - Proposed enhanced YAML
11. `11_ork_proposed_hybrid.py` - Proposed hybrid with optional Pydantic

## Next Steps

1. **Decision:** Which approach to implement first?
   - Recommend: Option 3 (Hybrid) with P0 focus on Python-first

2. **Prototype:** Implement @task decorator with auto-schema generation
   - Start with simple dict returns
   - Add TypedDict support
   - Keep Pydantic optional

3. **Validate:** Test with real examples
   - Port examples/demo to new style
   - Compare ergonomics
   - Gather feedback

4. **Iterate:** Add enhancements based on learnings
   - Inline YAML scripts
   - Built-in task types
   - Better templating
