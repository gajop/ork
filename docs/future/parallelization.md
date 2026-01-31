Status: Pending Review

# Parallelization

Ork supports three parallelization patterns: static, dynamic by count, and dynamic by items (foreach).

## Static Parallelization

Run a task with a fixed number of instances:

```yaml
tasks:
  process:
    executor: python
    file: tasks/process.py
    parallel: 4
```

Creates 4 instances with:
- `parallel_index`: 0, 1, 2, 3
- `parallel_count`: 4

### Task Implementation

```py
from ork import task, Context
from pydantic import BaseModel

class Input(BaseModel):
    pass

class Output(BaseModel):
    processed: int

@task
def main(input: Input, ctx: Context) -> Output:
    # Divide work by index
    all_items = get_all_items()
    my_items = [
        item for i, item in enumerate(all_items)
        if i % ctx.parallel_count == ctx.parallel_index
    ]

    results = process_items(my_items)

    ctx.log.info(
        f"Instance {ctx.parallel_index}/{ctx.parallel_count} "
        f"processed {len(results)} items"
    )

    return Output(processed=len(results))
```

## Dynamic by Count

An upstream task determines the instance count:

```yaml
tasks:
  discover:
    executor: python
    file: tasks/discover.py

  process:
    executor: python
    file: tasks/process.py
    depends_on: [discover]
    parallel: discover.count
```

### Discovery Task

```py
# tasks/discover.py
from ork import task
from pydantic import BaseModel

class Output(BaseModel):
    count: int

@task
def main() -> Output:
    count = count_items_to_process()
    return Output(count=count)
```

After `discover` completes, Ork reads `count` from the output and creates that many `process` instances.

## Dynamic by Items (Foreach)

One instance per item in a list:

```yaml
tasks:
  discover:
    executor: python
    file: tasks/discover.py

  process:
    executor: python
    file: tasks/process.py
    depends_on: [discover]
    foreach: discover.files
    concurrency: 8

  aggregate:
    executor: python
    file: tasks/aggregate.py
    depends_on: [process]
```

### Discovery Task

```py
# tasks/discover.py
from ork import task
from pydantic import BaseModel
from google.cloud import storage

class Output(BaseModel):
    files: list[str]

@task
def main() -> Output:
    client = storage.Client()
    blobs = client.list_blobs("my-bucket", prefix="raw/2024-01-15/")
    files = [f"gs://my-bucket/{b.name}" for b in blobs]
    return Output(files=files)
```

### Process Task

Each instance receives one item:

```py
# tasks/process.py
from ork import task, Context
from pydantic import BaseModel
import pandas as pd

class Input(BaseModel):
    item: str  # Automatically populated with one file path

class Output(BaseModel):
    input_file: str
    output_file: str
    rows: int

@task
def main(input: Input, ctx: Context) -> Output:
    df = pd.read_parquet(input.item)
    df = transform(df)

    output_path = input.item.replace("/raw/", "/processed/")
    df.to_parquet(output_path)

    ctx.metrics.record("rows_processed", len(df))

    return Output(
        input_file=input.item,
        output_file=output_path,
        rows=len(df)
    )
```

### Aggregate Task

Downstream tasks receive all outputs as a list:

```py
# tasks/aggregate.py
from ork import task
from pydantic import BaseModel

class ProcessOutput(BaseModel):
    input_file: str
    output_file: str
    rows: int

class Input(BaseModel):
    process: list[ProcessOutput]  # All process outputs

class Output(BaseModel):
    total_rows: int
    files_processed: int

@task
def main(input: Input) -> Output:
    return Output(
        total_rows=sum(p.rows for p in input.process),
        files_processed=len(input.process)
    )
```

## Concurrency Limits

Limit how many instances run simultaneously:

```yaml
tasks:
  process:
    executor: python
    file: tasks/process.py
    foreach: discover.files
    concurrency: 4
```

If `discover.files` has 100 items, only 4 instances run at a time. Others remain `pending` until a slot opens.

## Use Cases

### Static Parallelization
- Known partition count (e.g., process data for 12 months)
- Fixed shard count for distributed processing

### Dynamic by Count
- Partition count determined at runtime
- Database table partitions
- Date ranges discovered dynamically

### Foreach
- File processing (unknown file count)
- API pagination (process each page)
- Per-customer or per-tenant processing
- Any list of items discovered at runtime

## Combining Patterns

```yaml
tasks:
  # Discover regions
  discover_regions:
    executor: python
    file: tasks/discover_regions.py

  # Process each region with parallelism
  process_region:
    executor: python
    file: tasks/process_region.py
    foreach: discover_regions.regions
    parallel: 4  # 4 workers per region

  # Aggregate all results
  aggregate:
    executor: python
    file: tasks/aggregate.py
    depends_on: [process_region]
```

This creates `len(regions) * 4` total instances, with each region processed by 4 workers.
