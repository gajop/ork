import time


def task_a():
    """First task - just prints and returns"""
    print("Task A: Starting processing")
    result = {"task": "A", "status": "completed", "timestamp": time.time()}
    print(f"Task A: Result = {result}")
    return result


def _resolve_upstream(data):
    if isinstance(data, dict) and "upstream" in data and isinstance(data["upstream"], dict):
        return data["upstream"]
    if isinstance(data, dict):
        return data
    return {}


def task_b(input=None):
    """Second task - depends on A, uses its output"""
    print("Task B: Starting processing")
    upstream = _resolve_upstream(input or {})
    task_a_result = upstream.get("task_a", {})
    print(f"Task B: Received from A: {task_a_result}")
    result = {"task": "B", "status": "completed", "timestamp": time.time(), "previous": task_a_result}
    print(f"Task B: Result = {result}")
    return result


def task_c(input=None):
    """Third task - depends on B, uses its output"""
    print("Task C: Starting processing")
    upstream = _resolve_upstream(input or {})
    task_b_result = upstream.get("task_b", {})
    print(f"Task C: Received from B: {task_b_result}")
    result = {"task": "C", "status": "completed", "timestamp": time.time(), "previous": task_b_result}
    print(f"Task C: Result = {result}")
    return result
