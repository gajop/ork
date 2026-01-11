Status: Pending Review

# dbt Integration

Import dbt projects to use models as tasks in your workflows.

## Basic Usage

```yaml
import:
  - dbt: ./transform

tasks:
  extract:
    executor: python
    file: tasks/extract.py

  dbt.staging.stg_users:
    depends_on: [extract]

  dbt.marts.order_facts: {}

  export:
    executor: python
    file: tasks/export.py
    depends_on: [dbt.marts.order_facts]
```

## How It Works

1. Ork parses your dbt project
2. Extracts models and their dependencies
3. Creates tasks for each model
4. Preserves internal dbt dependencies
5. Allows adding external dependencies

## Task Naming

dbt models become tasks with the naming pattern:

```
dbt.<folder>.<model_name>
```

Examples:
- `models/staging/stg_users.sql` → `dbt.staging.stg_users`
- `models/marts/core/dim_customers.sql` → `dbt.marts.core.dim_customers`

## Adding Dependencies

You can add dependencies between dbt models and other tasks:

```yaml
tasks:
  # Custom Python task
  extract_data:
    executor: python
    file: tasks/extract.py

  # dbt staging models depend on extraction
  dbt.staging.stg_users:
    depends_on: [extract_data]

  dbt.staging.stg_orders:
    depends_on: [extract_data]

  # dbt mart models run after staging (internal dbt deps preserved)
  dbt.marts.order_facts: {}

  # Custom task depends on dbt output
  send_report:
    executor: python
    file: tasks/report.py
    depends_on: [dbt.marts.order_facts]
```

## Partial Imports

Import specific models or folders:

```yaml
import:
  - dbt: ./transform
    models:
      - staging.*        # All staging models
      - marts.core.*     # All core mart models
      - marts.fct_sales  # Specific model
```

## Configuration

Configure dbt execution:

```yaml
executors:
  dbt:
    type: dbt
    config:
      profiles_dir: ~/.dbt
      target: prod
      threads: 4
```

## Execution

When a dbt task runs, Ork executes:

```bash
dbt run --models <model_name> --profiles-dir <profiles_dir> --target <target>
```

## Testing

Include dbt tests in your workflow:

```yaml
tasks:
  dbt.staging.stg_users: {}

  test_stg_users:
    executor: dbt-test
    config:
      models: [staging.stg_users]
    depends_on: [dbt.staging.stg_users]
```

## Seeds and Snapshots

```yaml
tasks:
  dbt_seed:
    executor: dbt-seed
    config:
      select: [country_codes, product_categories]

  dbt.staging.stg_users:
    depends_on: [dbt_seed]

  snapshot_users:
    executor: dbt-snapshot
    config:
      select: [users_snapshot]
    depends_on: [dbt.staging.stg_users]
```
