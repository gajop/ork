"""Business logic for retryable operations"""
import os
import time


def attempt_operation(fail_times: int) -> dict:
    """Simulates a flakey operation that fails N times before succeeding"""
    attempt = int(os.environ.get("ORK_ATTEMPT", "1"))

    # Simulate work
    time.sleep(2)

    if attempt <= fail_times:
        raise RuntimeError(f"Intentional failure on attempt {attempt}")

    return {"ok": True, "attempt": attempt}
