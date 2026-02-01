import time


def task_a():
    """First task - just prints and returns"""
    print("Task A: Starting processing")
    result = {"task": "A", "status": "completed", "timestamp": time.time()}
    print(f"Task A: Result = {result}")
    return result


def task_b(upstream):
    """Second task - depends on A, uses its output"""
    print("Task B: Starting processing")
    task_a_result = upstream.get("task_a", {})
    print(f"Task B: Received from A: {task_a_result}")
    result = {"task": "B", "status": "completed", "timestamp": time.time(), "previous": task_a_result}
    print(f"Task B: Result = {result}")
    return result


def task_c(upstream):
    """Third task - depends on B, uses its output"""
    print("Task C: Starting processing")
    task_b_result = upstream.get("task_b", {})
    print(f"Task C: Received from B: {task_b_result}")
    result = {"task": "C", "status": "completed", "timestamp": time.time(), "previous": task_b_result}
    print(f"Task C: Result = {result}")
    return result
