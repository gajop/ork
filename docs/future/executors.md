Status: Pending Review

# Executors

Executors run tasks. Built-in executors handle Python and Rust. Custom executors handle domain-specific patterns.

## Built-in Executors

```yaml
# ork.yaml
executors:
  python:
    type: python
  rust:
    type: rust
```

## Custom Executors

Custom executors implement initialization and execution logic. They're useful for:
- Running SQL queries
- Making HTTP requests
- Running dbt models
- Domain-specific task patterns

### Example: BigQuery Executor

An executor that runs SQL, logs bytes processed, and estimates cost.

```py
# executors/bigquery.py
from ork import executor, Context
from pydantic import BaseModel
from google.cloud import bigquery
from pathlib import Path
from jinja2 import Template
import yaml

class BigQueryConfig(BaseModel):
    project: str
    location: str = "US"
    cost_per_tb: float = 5.0

class TaskConfig(BaseModel):
    file: str
    params: dict[str, str | int | float | bool] = {}

class TaskOutput(BaseModel):
    rows_affected: int
    bytes_processed: int
    estimated_cost_usd: float

@executor
class BigQueryExecutor:
    def __init__(self, config: BigQueryConfig):
        self.client = bigquery.Client(
            project=config.project,
            location=config.location
        )
        self.cost_per_tb = config.cost_per_tb

    def run(self, config_file: str, ctx: Context, upstream: dict[str, object] = {}) -> TaskOutput:
        task_config = TaskConfig(**yaml.safe_load(Path(config_file).read_text()))

        sql_template = Path(task_config.file).read_text()
        template = Template(sql_template)
        context = {**task_config.params, **upstream}
        sql = template.render(**context)

        job = self.client.query(sql)
        result = job.result()

        bytes_processed = job.total_bytes_processed or 0
        tb_processed = bytes_processed / (1024 ** 4)
        cost = tb_processed * self.cost_per_tb

        ctx.metrics.record("bytes_processed", bytes_processed, labels={
            "task": ctx.task_name,
            "workflow": ctx.workflow_name,
        })

        return TaskOutput(
            rows_affected=result.total_rows or 0,
            bytes_processed=bytes_processed,
            estimated_cost_usd=round(cost, 4)
        )
```

### Register the Executor

```yaml
# ork.yaml
executors:
  python:
    type: python

  bigquery:
    type: executors.bigquery:BigQueryExecutor
    config:
      project: my-gcp-project
      location: US
      cost_per_tb: 5.0
```

### Task Config

```yaml
# sql/staging/users.yaml
file: sql/staging/users.sql
params:
  days_back: 7
  status: active
  target_table: staging.users
```

### SQL Template

```sql
-- sql/staging/users.sql
CREATE OR REPLACE TABLE {{ target_table }} AS
SELECT *
FROM source.users
WHERE updated_at > DATE_SUB(CURRENT_DATE(), INTERVAL {{ days_back }} DAY)
  AND status = '{{ status }}'
```

### Workflow

```yaml
name: warehouse_refresh

tasks:
  stage_users:
    executor: bigquery
    config_file: sql/staging/users.yaml

  stage_orders:
    executor: bigquery
    config_file: sql/staging/orders.yaml

  build_facts:
    executor: bigquery
    config_file: sql/warehouse/facts.yaml
    depends_on: [stage_users, stage_orders]
```

## Executor Interface

### Python

```py
from ork import executor, Context
from pydantic import BaseModel

class ExecutorConfig(BaseModel):
    # Executor configuration fields
    pass

class TaskOutput(BaseModel):
    # Task output fields
    pass

@executor
class MyExecutor:
    def __init__(self, config: ExecutorConfig):
        # Initialize executor with config
        pass

    def run(self, config_file: str, ctx: Context, upstream: dict[str, object] = {}) -> TaskOutput:
        # Execute task
        # - config_file: path to task config file
        # - ctx: execution context
        # - upstream: outputs from upstream tasks
        pass
```

### Rust

```rust
use ork::{executor, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct ExecutorConfig {
    // Executor configuration fields
}

#[derive(Serialize)]
struct TaskOutput {
    // Task output fields
}

#[executor]
struct MyExecutor {
    // Executor state
}

impl MyExecutor {
    fn new(config: ExecutorConfig) -> Result<Self> {
        // Initialize executor
    }

    fn run(&self, config_file: &str, ctx: &Context, upstream: &serde_json::Value) -> Result<TaskOutput> {
        // Execute task
    }
}
```
