Status: Pending Review

# Type Schemas

Type schemas provide validation and enable UI visualization of data flowing through workflows.

## Declaring Types

Define types in the workflow YAML:

```yaml
types:
  User:
    id: int
    name: str
    email: str
    created_at: datetime

  ExtractOutput:
    users: list[User]
    count: int

tasks:
  extract_users:
    executor: python
    file: tasks/extract_users.py
    output: ExtractOutput

  transform_users:
    executor: python
    file: tasks/transform.py
    depends_on: [extract_users]
```

## Type Validation

When a task completes, Ork validates the output against the declared schema:

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    output: ExtractOutput
```

If the output doesn't match `ExtractOutput`, the task fails with a validation error.

## Supported Types

### Primitives

- `int` - Integer
- `float` - Floating point
- `str` - String
- `bool` - Boolean
- `datetime` - ISO 8601 datetime

### Collections

- `list[T]` - List of type T
- `dict[K, V]` - Dictionary with key type K and value type V

### Optional

- `T | None` - Optional type (can be null)
- `T?` - Shorthand for optional

### Nested Types

```yaml
types:
  Address:
    street: str
    city: str
    country: str

  User:
    id: int
    name: str
    email: str
    address: Address | None
```

## Exporting from Code

Generate schemas from Pydantic models:

```bash
ork schema export tasks/extract.py
```

Output:

```yaml
types:
  User:
    id: int
    name: str
    email: str

  ExtractOutput:
    users: list[User]
```

Add to your workflow definition.

## Benefits

### Validation

Catch type mismatches early:

```py
# tasks/extract.py
class Output(BaseModel):
    users: list[User]

@task
def main() -> Output:
    # BUG: returning wrong type
    return Output(users="invalid")  # Validation fails
```

### Documentation

Types serve as documentation for task inputs/outputs:

```yaml
types:
  TransformInput:
    extract: ExtractOutput
    config:
      normalize: bool
      remove_duplicates: bool

tasks:
  transform:
    file: tasks/transform.py
    input: TransformInput
```

Developers know exactly what the task expects.

### UI Visualization

Type schemas enable the UI to:
- Display data shapes
- Render sample data
- Show type errors
- Generate input forms

## Runtime Schema Evolution

Update schemas without breaking existing runs:

```yaml
types:
  User:
    id: int
    name: str
    email: str
    # New field (optional to maintain compatibility)
    phone: str | None
```

Existing runs with old schema continue to work. New runs use new schema.

## Validation Errors

When validation fails:

```
Task extract_users failed: Output validation error
Expected: ExtractOutput
Got: {"users": ["invalid"], "count": 1}
Error: users: expected list[User], got list[str]
```

Validation errors include:
- Expected type
- Actual value
- Specific field that failed

## Disabling Validation

For development, disable validation:

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    output: ExtractOutput
    validate_output: false
```

Or globally:

```yaml
# ork.yaml
validation:
  enabled: false
```
