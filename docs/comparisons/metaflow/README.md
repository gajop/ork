# Metaflow Examples

Netflix's ML/AI workflow framework.

## Setup

```bash
uv sync
```

## Usage

```bash
# Run flows
just run parallel_tasks
just run data_pipeline
just run conditional_branching

# Run all
just run-all

# List recent runs
just list

# Start UI (optional)
just server
# Open http://localhost:8324
```

## Examples

- `flows/parallel_tasks.py`
- `flows/data_pipeline.py`
- `flows/conditional_branching.py`

## Documentation

[Metaflow Docs](https://docs.metaflow.org/)
