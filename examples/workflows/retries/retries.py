"""Retries workflow - consolidated single-file example"""
import os
import time
from pydantic import BaseModel


# Business logic functions
def create_greeting(name: str, delay: float = 0) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    return {"message": message, "timestamp": time.time()}


def attempt_operation(fail_times: int) -> dict:
    """Simulates a flakey operation that fails N times before succeeding"""
    attempt = int(os.environ.get("ORK_ATTEMPT", "1"))

    # Simulate work
    time.sleep(2)

    if attempt <= fail_times:
        raise RuntimeError(f"Intentional failure on attempt {attempt}")

    return {"ok": True, "attempt": attempt}


# Hello task
class HelloInput(BaseModel):
    name: str = "world"
    delay: float = 1.0


class HelloOutput(BaseModel):
    message: str
    timestamp: float


def hello(input: HelloInput) -> HelloOutput:
    result = create_greeting(input.name, input.delay)
    print(result["message"])
    return HelloOutput(message=result["message"], timestamp=result["timestamp"])


# Flakey task
class FlakeyInput(BaseModel):
    fail_times: int = 0


class FlakeyOutput(BaseModel):
    ok: bool
    attempt: int


def flakey(input: FlakeyInput) -> FlakeyOutput:
    result = attempt_operation(input.fail_times)
    print(f"succeeded on attempt {result['attempt']}")
    return FlakeyOutput(ok=result["ok"], attempt=result["attempt"])
