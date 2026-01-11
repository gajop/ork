# Design Explorations - Making Ork Production-Ready

## Executive Summary

**Current State:** Ork requires ~70 lines across 4 files for a 3-task workflow. This is **2-3x more verbose** than competitors and makes it a toy, not a production tool.

**The Fix:** Implement functional `@task` decorator with auto-schema generation → **reduces to ~25-30 lines in 1 file**.

**Implementation Priority:** P0 - This is blocking Ork from being usable.

## Critical Problems

1. **@task decorator does nothing** - just documentation, not functional
2. **Pydantic verbosity** - requires 2+ BaseModel classes even for trivial tasks
3. **Schema duplication** - downstream tasks must redefine upstream output schemas
4. **Too many files** - every task needs its own file
5. **Generic class names** - `Input`/`Output` everywhere, unsearchable
6. **High barrier to entry** - simple workflows require tons of boilerplate

## Line Count Comparison (3-task workflow)

```
Ork Current:      70 lines, 4 files  ████████████████████
Temporal:         80 lines, 1 file   ████████████████████████
Dagster:          55 lines, 1 file   ██████████████
GCP Workflows:    60 lines, 1 file   ███████████████
Kestra:           45 lines, 1 file   ███████████
Airflow:          45 lines, 1 file   ███████████
Prefect:          35 lines, 1 file   █████████
Mage:             35 lines, 1 file   █████████
Ork Proposed:     25 lines, 1 file   ██████
```

**Insight:** Ork is currently tied for WORST verbosity with Temporal, but Temporal justifies it with durable execution guarantees. Ork has no excuse.

## Exploration 1: Pure Python with @task Decorator (Dagster-style)

### Current Ork approach (3 files):
```
examples/demo/
├── demo.yaml              # Workflow definition
├── tasks/
│   ├── hello.py          # Task 1 (20 lines for simple greeting)
│   └── compliment.py     # Task 2 (25 lines for simple string concat)
└── src/
    └── greetings.py      # Business logic
```

### Alternative: Single Python file, no YAML

```python
# demo_workflow.py
from ork import task, workflow
import time

@task
def hello(name: str = "world", delay: float = 1.0) -> dict:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@task(depends_on=[hello])
def compliment(hello: dict, adjective: str = "fast") -> dict:
    """Create a compliment based on a greeting"""
    time.sleep(5)
    line = f"{hello['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@workflow
def demo():
    """Define the workflow"""
    h = hello(name="Ork dev", delay=5)
    c = compliment(hello=h, adjective="ridiculously fast")
    return c

if __name__ == "__main__":
    # Run locally
    demo.run()
```

**Running it:**
```bash
ork run demo_workflow.py
# or
python demo_workflow.py
```

**Pros:**
- Single file, ~30 lines total vs 3 files with ~60+ lines
- No Pydantic boilerplate
- No schema duplication (Python type hints are enough)
- @task actually does something
- Natural Python - feels like writing normal functions
- Can still separate business logic if needed (just import it)

**Cons:**
- Locks you into Python (can't easily mix shell/dbt/etc)
- Less declarative (harder to visualize DAG without running)
- Workflow definition is code, not config

### Alternative 1b: Hybrid - Python tasks, explicit DAG

```python
# demo_workflow.py
from ork import task, DAG

@task
def hello(name: str = "world", delay: float = 1.0) -> dict:
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@task
def compliment(hello_output: dict, adjective: str = "fast") -> dict:
    time.sleep(5)
    line = f"{hello_output['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

# Define DAG
dag = DAG("demo")

dag.add_task(
    hello,
    name="greet",
    name="Ork dev",
    delay=5
)

dag.add_task(
    compliment,
    name="compliment",
    depends_on=["greet"],
    adjective="ridiculously fast"
)
```

## Exploration 2: Pure YAML (Zero Python by Default)

### Idea: Make YAML powerful enough that most workflows don't need code

```yaml
# demo.yaml
name: local_demo

tasks:
  greet:
    run: |
      echo "Hello, {{ inputs.name }}!"
      sleep {{ inputs.delay }}
    inputs:
      name: "Ork dev"
      delay: 5
    outputs:
      message: "Hello, {{ inputs.name }}!"
      timestamp: "{{ now() }}"

  compliment:
    run: |
      echo "{{ tasks.greet.outputs.message }} — you built Ork to be {{ inputs.adjective }}!"
      sleep 5
    depends_on: [greet]
    inputs:
      adjective: "ridiculously fast"
    outputs:
      line: "{{ tasks.greet.outputs.message }} — you built Ork to be {{ inputs.adjective }}!"

  # When you DO need Python, reference it inline
  complex_analytics:
    executor: python
    run: |
      import pandas as pd
      df = pd.read_sql(...)
      return {"count": len(df)}
    # or
    file: tasks/analytics.py
```

**Built-in task types:**
```yaml
tasks:
  fetch_api:
    type: http
    url: https://api.github.com/repos/{{ inputs.repo }}
    method: GET
    headers:
      Authorization: "token {{ secrets.github_token }}"
    outputs:
      data: "{{ response.json }}"

  load_to_db:
    type: sql
    connection: postgres://...
    query: |
      INSERT INTO users (id, name)
      SELECT id, name FROM {{ tasks.fetch_api.outputs.data }}

  transform_dbt:
    type: dbt
    project: ./dbt_project
    models: [staging.users, marts.daily_active]

  send_slack:
    type: slack
    webhook: "{{ secrets.slack_webhook }}"
    message: "Pipeline completed! Processed {{ tasks.load_to_db.outputs.rows }} rows"
```

**Pros:**
- No code required for 80% of data pipelines
- Very declarative - easy to visualize
- Templating makes it powerful (Jinja2-style)
- Can drop to code when needed
- Familiar to Airflow/Prefect users

**Cons:**
- YAML can get complex (templating logic in strings)
- Harder to test/debug
- Less type safety
- Need to build many task types

## Exploration 3: Comparison with Popular Tools

### Example workflow: 3-stage ELT pipeline
- Extract from API
- Transform with pandas
- Load to database

### Airflow
```python
# dags/elt_pipeline.py (60+ lines)
from airflow import DAG
from airflow.operators.python import PythonOperator
from datetime import datetime

def extract(**context):
    import requests
    data = requests.get("https://api.example.com/data").json()
    return data

def transform(**context):
    import pandas as pd
    data = context['ti'].xcom_pull(task_ids='extract')
    df = pd.DataFrame(data)
    df['processed_at'] = pd.Timestamp.now()
    return df.to_dict('records')

def load(**context):
    import psycopg2
    data = context['ti'].xcom_pull(task_ids='transform')
    conn = psycopg2.connect(...)
    # ... insert logic

with DAG(
    'elt_pipeline',
    start_date=datetime(2024, 1, 1),
    schedule_interval='@daily',
    catchup=False
) as dag:

    extract_task = PythonOperator(
        task_id='extract',
        python_callable=extract
    )

    transform_task = PythonOperator(
        task_id='transform',
        python_callable=transform
    )

    load_task = PythonOperator(
        task_id='load',
        python_callable=load
    )

    extract_task >> transform_task >> load_task
```

**Pros:** Mature, huge ecosystem, scheduling built-in
**Cons:** Heavy (needs server), verbose, XCOM is clunky, hard to test

### Prefect
```python
# elt_pipeline.py (40 lines)
from prefect import flow, task
import requests
import pandas as pd

@task
def extract():
    return requests.get("https://api.example.com/data").json()

@task
def transform(data):
    df = pd.DataFrame(data)
    df['processed_at'] = pd.Timestamp.now()
    return df.to_dict('records')

@task
def load(data):
    import psycopg2
    conn = psycopg2.connect(...)
    # ... insert logic

@flow(name="elt_pipeline")
def elt_pipeline():
    data = extract()
    transformed = transform(data)
    load(transformed)

if __name__ == "__main__":
    elt_pipeline()
```

**Pros:** Pythonic, easy to test, good typing, can run locally
**Cons:** Needs Prefect Cloud/Server for scheduling, not as lightweight

### Dagster
```python
# elt_pipeline.py (50 lines)
from dagster import asset, Definitions
import requests
import pandas as pd

@asset
def raw_data():
    """Extract data from API"""
    return requests.get("https://api.example.com/data").json()

@asset
def transformed_data(raw_data):
    """Transform with pandas"""
    df = pd.DataFrame(raw_data)
    df['processed_at'] = pd.Timestamp.now()
    return df

@asset
def loaded_data(transformed_data):
    """Load to database"""
    import psycopg2
    conn = psycopg2.connect(...)
    # ... insert logic
    return len(transformed_data)

defs = Definitions(assets=[raw_data, transformed_data, loaded_data])
```

**Pros:** Asset-based (data-first), great UI, strong typing
**Cons:** Complex concepts, needs server, heavy

### dbt (pure SQL/YAML)
```yaml
# models/schema.yml
models:
  - name: raw_data
    config:
      materialized: table
  - name: transformed_data
    config:
      materialized: table

# models/raw_data.sql
{{ config(materialized='table') }}

SELECT * FROM external_table('https://api.example.com/data', format='json')

# models/transformed_data.sql
{{ config(materialized='table') }}

SELECT
    *,
    CURRENT_TIMESTAMP as processed_at
FROM {{ ref('raw_data') }}
```

**Pros:** Pure SQL (data teams love it), refs are powerful, compiled
**Cons:** SQL-only, needs external orchestration, not for general tasks

### Temporal (workflow engine)
```python
# elt_workflow.py (70+ lines)
from temporalio import workflow, activity
from datetime import timedelta

@activity.defn
async def extract():
    import requests
    return requests.get("https://api.example.com/data").json()

@activity.defn
async def transform(data):
    import pandas as pd
    df = pd.DataFrame(data)
    df['processed_at'] = pd.Timestamp.now()
    return df.to_dict('records')

@activity.defn
async def load(data):
    import psycopg2
    conn = psycopg2.connect(...)
    # ... insert logic

@workflow.defn
class ETLWorkflow:
    @workflow.run
    async def run(self):
        data = await workflow.execute_activity(
            extract,
            start_to_close_timeout=timedelta(minutes=5)
        )
        transformed = await workflow.execute_activity(
            transform,
            data,
            start_to_close_timeout=timedelta(minutes=5)
        )
        await workflow.execute_activity(
            load,
            transformed,
            start_to_close_timeout=timedelta(minutes=5)
        )
```

**Pros:** Durable execution, retries/compensation built-in, scales
**Cons:** Very complex, needs server, async everywhere, overkill for batch

### Current Ork (typed approach)
```python
# tasks/extract.py (15 lines)
from pydantic import BaseModel
import requests

class ExtractInput(BaseModel):
    url: str

class ExtractOutput(BaseModel):
    data: list

def main(input: ExtractInput) -> ExtractOutput:
    data = requests.get(input.url).json()
    return ExtractOutput(data=data)

# tasks/transform.py (20 lines)
from pydantic import BaseModel
import pandas as pd

class ExtractOutput(BaseModel):  # Duplicate!
    data: list

class TransformInput(BaseModel):
    extract: ExtractOutput

class TransformOutput(BaseModel):
    records: list

def main(input: TransformInput) -> TransformOutput:
    df = pd.DataFrame(input.extract.data)
    df['processed_at'] = pd.Timestamp.now()
    return TransformOutput(records=df.to_dict('records'))

# elt.yaml (15 lines)
name: elt_pipeline
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    input:
      url: "https://api.example.com/data"

  transform:
    executor: python
    file: tasks/transform.py
    depends_on: [extract]
```

**Total: 3 files, 50+ lines for just 2 tasks**

### Proposed Ork (Python-first)
```python
# elt_pipeline.py (25 lines)
from ork import task, workflow
import requests
import pandas as pd

@task
def extract(url: str) -> dict:
    data = requests.get(url).json()
    return {"data": data}

@task
def transform(extract: dict) -> dict:
    df = pd.DataFrame(extract["data"])
    df['processed_at'] = pd.Timestamp.now()
    return {"records": df.to_dict('records')}

@task
def load(transform: dict) -> dict:
    import psycopg2
    conn = psycopg2.connect(...)
    # ... insert logic
    return {"count": len(transform["records"])}

@workflow
def elt_pipeline():
    e = extract(url="https://api.example.com/data")
    t = transform(e)
    l = load(t)
```

**Total: 1 file, 25 lines**

## Comparison Table

| Tool | File Count | Line Count | Type Safety | Local Dev | Server Required | Verbosity |
|------|-----------|------------|-------------|-----------|-----------------|-----------|
| **Airflow** | 1 | 60+ | Low | Hard | Yes | High |
| **Prefect** | 1 | 40 | Medium | Easy | Optional | Medium |
| **Dagster** | 1 | 50 | High | Easy | Yes | Medium |
| **dbt** | 3+ | 30 | N/A (SQL) | Easy | No | Low |
| **Temporal** | 1 | 70+ | High | Hard | Yes | Very High |
| **Ork (current)** | 3+ per task | 50+ for 2 tasks | High | Easy | No | **Very High** |
| **Ork (proposed)** | 1 | 25 | Medium | Easy | No | **Low** |

## Key Insights from Tool Comparison

See working examples in this directory and authoritative links in `authoritative_examples.md`.

### 0. Market Adoption & Popularity

**GitHub Stars (January 2026):**
- [Airflow](https://github.com/apache/airflow): ⭐ 43.8k - Most mature, Apache Foundation
- [Kestra](https://github.com/kestra-io/kestra): ⭐ 26.2k - Fast-growing YAML-first tool
- [Prefect](https://github.com/PrefectHQ/prefect): ⭐ 21.3k - Modern Python-first alternative
- [Temporal](https://github.com/temporalio/temporal): ⭐ 17.5k - Durable execution specialist
- [Dagster](https://github.com/dagster-io/dagster): ⭐ 14.7k - Asset-based lineage
- [Mage](https://github.com/mage-ai/mage-ai): ⭐ 8.6k - Data engineering focused

**Key Insight:** Kestra (YAML-first) and Prefect (Python-first) are growing fastest. Both patterns are viable.

### 1. Decorator Pattern is Universal
**EVERY modern tool uses functional decorators:**
- Airflow: `@task` / `@dag`
- Prefect: `@task` / `@flow`
- Dagster: `@asset` / `@op`
- Temporal: `@activity` / `@workflow`
- Mage: `@data_loader` / `@transformer`

**Ork is the ONLY tool where `@task` is non-functional.**

### 2. Schema Duplication is Unique to Ork

No other tool requires you to redefine upstream outputs:

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

### 3. File Proliferation is an Anti-Pattern

**File count for 3-task workflow:**
- Most tools: 1 file
- Ork: 4 files (workflow.yaml + 3 task files)

### 4. Two Dominant Patterns Emerge

**Pattern A: Python-First** (Prefect, modern Airflow, Mage)
- Best for: Pure Python workflows
- Ergonomics: ⭐⭐⭐⭐⭐ (30-40 lines)
- Flexibility: ⭐⭐⭐ (Python-only)

**Pattern B: YAML-First** (GCP Workflows, Kestra, dbt)
- Best for: Mixed executors, declarative
- Ergonomics: ⭐⭐⭐ (45-60 lines, inline scripts help)
- Flexibility: ⭐⭐⭐⭐⭐ (any language)

**Ork tries to be both but currently fails at ergonomics for BOTH.**

### 5. Ork's Serverless Advantage is Being Squandered

**Server Requirements:**
- Requires server: Airflow, Dagster, Temporal, Mage, Kestra
- **No server: Ork, Prefect (optional), GCP Workflows (managed)**

**Ork's serverless architecture is a MAJOR differentiator. We're killing it with bad ergonomics.**

## Recommendations

### Option A: Python-first with @workflow decorator
- Most ergonomic for 80% of use cases
- Can still use YAML for complex multi-executor scenarios
- Backwards compatible (keep YAML support)

### Option B: Hybrid - reduce Pydantic verbosity
- Make Input/Output classes optional
- Auto-generate them from type hints
- Allow dict returns for simple cases

### Option C: Make YAML more powerful
- Add built-in task types (http, sql, dbt, etc)
- Support inline Python/shell scripts
- Better templating

### My recommendation: **Option A + C**
1. Make @task decorator functional (Python-first for simple workflows)
2. Keep YAML for complex multi-language workflows
3. Add built-in task types to YAML to reduce need for custom tasks
4. Let users choose: pure Python OR pure YAML OR hybrid
