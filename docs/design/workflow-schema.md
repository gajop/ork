# Workflow Schema

Status: draft implementation with CLI validation support.

## Scope
This document defines a strict workflow schema for YAML task definitions.

Current CLI support:
- Validate command: `ork validate-workflow <file.yaml>`
- Validation is strict and intentionally rejects v1-style `upstream` conventions.

## Design goals
- No implicit `upstream` injection.
- No untyped/bare input values in task bindings.
- Explicit input/output typing for every task.
- Deterministic, cross-language validation behavior.

## Top-level schema
```yaml
name: <workflow_name>

params:               # optional
  <param_name>:
    type: <type_expr>
    required: true|false
    default: <literal>   # required when required=false

types:                # optional aliases
  <TypeAlias>: <type_expr>

tasks:
  <task_name>:
    executor: python|process|cloudrun|library
    file: <path>         # executor-dependent
    module: <module>     # executor-dependent
    function: <name>     # optional for python
    command: <cmd>       # executor-dependent
    job: <job_name>      # executor-dependent
    depends_on: [<task_name>, ...]
    run_if:              # optional
      deps: all_success|all_done|any_failed
    input_type: <object_type_expr>
    output_type: <type_expr>
    inputs:
      <arg_name>: <binding>
```

## Type system
Allowed builtins:
- `str`
- `int`
- `float`
- `bool`
- `date`
- `datetime_notz`
- `datetime_tz`
- `datetime_utc`

Legacy names are rejected:
- `string`, `integer`, `number`, `boolean`, `datetime`, `timestamp`, `timestamp_s`, `timestamp_ms`, `time`, `duration`

Type expression grammar:
- builtin: `<builtin>`
- alias: `types.<Alias>`
- object: `{ field_a: <type_expr>, field_b: <type_expr> }`
- array: `[<type_expr>]` (exactly one element type)

Rules:
- `input_type` and `output_type` are type-only declarations (no `const`/`ref`).
- `input_type` must resolve to an object type (named task arguments).
- Type aliases must not use builtin names.
- Recursive alias cycles are invalid.

## Temporal formats
- `date`: `YYYY-MM-DD`
- `datetime_notz`: `YYYY-MM-DDTHH:MM:SS[.fraction]` (timezone/offset not allowed)
- `datetime_tz`: RFC3339 datetime with timezone (`Z` or `+/-HH:MM`)
- `datetime_utc`: RFC3339 datetime with strict `Z` suffix

Compatibility:
- `datetime_utc` is assignable where `datetime_tz` is expected.
- No other implicit temporal coercions.

## Input bindings
A binding must be exactly one of:

```yaml
<arg>:
  const: <literal>
```

```yaml
<arg>:
  ref: <path>
```

No bare scalar/array/object values are allowed in `inputs`.

Reference path grammar:
- `params.<param_name>[.<field>...]`
- `tasks.<task_name>.output[.<field>...]`

Rules:
- Task output refs are only valid if the current task has `depends_on` that task.
- Self-reference (`tasks.<current>.output`) is invalid.
- Ref field traversal must follow object fields.
- Ref type must be assignable to the target input field type.

## Task graph and execution gating
- `depends_on` tasks must exist.
- No self-dependency.
- No duplicate dependencies.
- Task graph must be acyclic.
- `run_if` is valid only when `depends_on` is non-empty.

## Executor-specific requirements
- `python`: non-empty `module` or `file`
- `process`: non-empty `command` or `file`
- `cloudrun`: non-empty `job`
- `library`: non-empty `file`

## Command
Validate a workflow file:

```bash
ork validate-workflow path/to/workflow.yaml
```

On success:
- Prints a success line with workflow name and task count.

On failure:
- Returns non-zero and reports the path/rule that failed.

## Valid example
```yaml
name: fast_demo

types:
  TaskResult:
    task: str
    status: str
    timestamp: datetime_utc

tasks:
  task_a:
    executor: python
    module: fast
    function: task_a
    input_type: {}
    output_type: types.TaskResult
    inputs: {}

  task_b:
    executor: python
    module: fast
    function: task_b
    depends_on: [task_a]
    input_type:
      a: types.TaskResult
      retries: int
    output_type: types.TaskResult
    inputs:
      a:
        ref: tasks.task_a.output
      retries:
        const: 1
```

## Non-goals in this version
- `expr` bindings
- Legacy schema compatibility mode
- Implicit input unpacking heuristics
