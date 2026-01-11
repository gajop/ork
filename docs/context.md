Status: Pending Review

# Context

Tasks receive a context object with execution metadata.

## Usage

```py
from ork import task, Context
from pydantic import BaseModel

class Input(BaseModel):
    item: str

class Output(BaseModel):
    processed: int

@task
def main(input: Input, ctx: Context) -> Output:
    rows = process(input.item)

    # Logging
    ctx.log.info(f"Processed {rows} rows")
    ctx.log.warning("Memory usage high")
    ctx.log.error("Validation failed")

    # Metrics
    ctx.metrics.record("rows_processed", rows)
    ctx.metrics.record("memory_mb", 512, labels={
        "task": ctx.task_name,
        "attempt": ctx.attempt
    })

    return Output(processed=rows)
```

## Available Fields

| Field | Type | Description |
|-------|------|-------------|
| `task_id` | `Uuid` | Unique task run identifier |
| `run_id` | `Uuid` | Workflow run identifier |
| `task_name` | `str` | Name of the task |
| `workflow_name` | `str` | Name of the workflow |
| `attempt` | `int` | Retry attempt number (starts at 1) |
| `parallel_index` | `int \| None` | Instance index for parallel tasks |
| `parallel_count` | `int \| None` | Total instances for parallel tasks |
| `log` | `Logger` | Structured logger |
| `metrics` | `MetricsRecorder` | Metrics recorder |

## Parallel Task Context

For parallel tasks, use `parallel_index` and `parallel_count` to partition work:

```py
@task
def main(input: Input, ctx: Context) -> Output:
    # Divide work across instances
    all_items = get_all_items()
    my_items = [
        item for i, item in enumerate(all_items)
        if i % ctx.parallel_count == ctx.parallel_index
    ]

    results = [process(item) for item in my_items]

    ctx.log.info(
        f"Instance {ctx.parallel_index}/{ctx.parallel_count} "
        f"processed {len(results)} items"
    )

    return Output(results=results)
```

## Rust Context

```rust
use ork::{task, Context};

#[task]
fn main(input: Input, ctx: Context) -> ork::Result<Output> {
    let rows = process(&input.item)?;

    ctx.log().info(&format!("Processed {} rows", rows));
    ctx.metrics().record("rows_processed", rows as f64, &[]);

    Ok(Output { processed: rows })
}
```
