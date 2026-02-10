"""Retries workflow - plain Python functions example"""
import os
import time


def flakey(fail_times: int = 0) -> dict:
    """Simulates a flakey operation that fails N times before succeeding"""
    attempt = int(os.environ.get("ORK_ATTEMPT", "1"))

    # Simulate work
    time.sleep(2)

    if attempt <= fail_times:
        raise RuntimeError(f"Intentional failure on attempt {attempt}")

    result = {"ok": True, "attempt": attempt}
    print(f"succeeded on attempt {result['attempt']}")
    return result


def hello(name: str = "world", delay: float = 1.0, upstream: dict = None) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result = {"message": message, "timestamp": time.time()}
    print(result["message"])
    return result
