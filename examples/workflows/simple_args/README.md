# Simple Arguments Example

This example demonstrates how to write Python tasks using standard function signatures, without needing Pydantic `Input` models.

## How it works

The python executor automatically unpacks the `input` JSON into the function arguments if the names match.

```python
# No need for class Input(BaseModel)!
def add(a: int, b: int):
    return a + b
```

## Running

```bash
just example simple_args
```
