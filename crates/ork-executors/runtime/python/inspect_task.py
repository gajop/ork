
import inspect
import importlib.util
import sys
import os
import json
from typing import Any, Dict, Optional


def main() -> int:
    """Entry point for the script."""
    if len(sys.argv) < 3:
        usage = {"error": "Usage: inspect_python_task.py <file_path> <function_name>"}
        print(json.dumps(usage), file=sys.stderr)
        return 1

    file_path = sys.argv[1]
    func_name = sys.argv[2]

    result = inspect_task(file_path, func_name)
    print(json.dumps(result))

    if "error" in result:
        print(result["error"], file=sys.stderr)
        return 1
    return 0


def inspect_task(path: str, func_name: str) -> Dict[str, Any]:
    """Inspects a Python task file and returns its signature."""
    _ensure_importable(path)

    module = _load_module(path)
    if isinstance(module, dict): # Error case
        return module

    function = _get_function(module, func_name)
    if isinstance(function, dict): # Error case
        return function

    return _extract_signature(function)


def _ensure_importable(path: str) -> None:
    """Adds the project root to sys.path to allow imports."""
    project_root = os.path.dirname(os.path.abspath(path))
    if project_root not in sys.path:
        sys.path.insert(0, project_root)


def _load_module(path: str) -> Any:
    """Loads a Python module from the given path."""
    spec = importlib.util.spec_from_file_location("ork_task_inspect", path)
    if not spec or not spec.loader:
        return {"error": f"Could not load file {path}"}

    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        return module
    except Exception as e:
        return {"error": f"Error executing module: {e}"}


def _get_function(module: Any, func_name: str) -> Any:
    """Retrieves a function from the loaded module."""
    fn = getattr(module, func_name, None)
    if not fn:
        return {"error": f"Function {func_name} not found"}
    return fn


def _extract_signature(fn: Any) -> Dict[str, Any]:
    """Extracts input and output types from a function signature."""
    try:
        sig = inspect.signature(fn)
    except Exception as e:
        return {"error": f"Could not get signature: {e}"}

    inputs = {}
    for name, param in sig.parameters.items():
        type_name = _get_type_name(param.annotation)
        if type_name is None:
             return {"error": f"Argument '{name}' is untyped"}
        inputs[name] = type_name

    output = _get_type_name(sig.return_annotation)

    return {
        "inputs": inputs,
        "output": output
    }


def _get_type_name(t: Any) -> Optional[str]:
    """Resolves a type annotation to a string name."""
    if t is inspect._empty:
        return None
    if hasattr(t, "__name__"):
        return t.__name__
    return str(t)


if __name__ == "__main__":
    sys.exit(main())
