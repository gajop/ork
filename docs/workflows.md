Status: Pending Review

# Workflows

A workflow is a directed acyclic graph (DAG) of tasks.

## Basic Structure

```yaml
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

## Execution Order

Tasks run in dependency order. Tasks without dependencies run in parallel.

In the example above:
1. `extract` runs first
2. `transform` runs after `extract` completes
3. `load` runs after `transform` completes

## Multiple Dependencies

```yaml
tasks:
  extract_users:
    executor: python
    file: tasks/extract_users.py

  extract_orders:
    executor: python
    file: tasks/extract_orders.py

  join:
    executor: python
    file: tasks/join.py
    depends_on: [extract_users, extract_orders]
```

`join` waits for both `extract_users` and `extract_orders` to complete successfully.

## Workflow Definition

```yaml
name: string              # required, unique identifier
schedule: string          # optional, cron expression

import:                   # optional, external sources
  - dbt: ./path           # import dbt project

types:                    # optional, type schemas
  TypeName:
    field: type

tasks:
  task_name:
    executor: string      # required, executor name
    file: string          # task file path
    config_file: string   # config file path (for custom executors)
    input: object         # static input values
    depends_on: [string]  # upstream task names
    timeout: int          # seconds, default 3600
    retries: int          # max retries, default 0
    retry_delay: int      # seconds between retries
    parallel: int | ref   # static count or reference
    foreach: ref          # reference to list field
    concurrency: int      # max parallel instances
    resources:
      cpu: int            # millicores or cores
      memory: string      # e.g. "4Gi"
    env:
      KEY: value          # literal value
      KEY:
        secret: ref       # secret manager reference
    on_failure: string    # "skip" (default) or "run"
    output: TypeName      # output type for validation
```
