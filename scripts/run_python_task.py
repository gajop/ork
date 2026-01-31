import importlib.util
import inspect
import json
import os
import sys


def main() -> int:
    path = os.environ.get("ORK_TASK_FILE")
    module_name = os.environ.get("ORK_TASK_MODULE")
    function_name = os.environ.get("ORK_TASK_FUNCTION", "main")
    if not path and not module_name:
        print("ORK_TASK_FILE or ORK_TASK_MODULE not set", file=sys.stderr)
        return 1

    input_json = os.environ.get("ORK_INPUT_JSON", "{}")
    try:
        input_data = json.loads(input_json) if input_json else {}
    except Exception as err:
        print(f"Failed to parse ORK_INPUT_JSON: {err}", file=sys.stderr)
        return 1

    if path:
        spec = importlib.util.spec_from_file_location("ork_task", path)
        if spec is None or spec.loader is None:
            print(f"Failed to load task file: {path}", file=sys.stderr)
            return 1
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        try:
            module = importlib.import_module(module_name)
        except Exception as err:
            print(f"Failed to import module {module_name}: {err}", file=sys.stderr)
            return 1

    fn = getattr(module, function_name, None)
    if fn is None:
        print(f"Task function not found: {function_name}", file=sys.stderr)
        return 1

    sig = inspect.signature(fn)
    args = []
    if len(sig.parameters) > 0:
        param = next(iter(sig.parameters.values()))
        ann = param.annotation
        if ann is not inspect._empty:
            try:
                if isinstance(input_data, dict):
                    input_obj = ann(**input_data)
                else:
                    input_obj = ann(input_data)
            except Exception:
                input_obj = input_data
        else:
            input_obj = input_data
        args.append(input_obj)

    result = fn(*args)

    if result is None:
        return 0

    if hasattr(result, "model_dump"):
        result = result.model_dump()
    elif hasattr(result, "dict"):
        result = result.dict()

    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
