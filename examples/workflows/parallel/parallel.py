"""Parallel workflow - TypedDict example"""
import time
from datetime import datetime
from typing import TypedDict


class HelloOutput(TypedDict):
    message: str
    timestamp: datetime


class ComplimentOutput(TypedDict):
    line: str
    adjective: str


def hello(name: str = "world", delay: float = 1.0) -> HelloOutput:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: HelloOutput = {"message": message, "timestamp": datetime.now()}
    print(result["message"])
    return result


def compliment(adjective: str = "fast", upstream: dict = None) -> ComplimentOutput:
    """Create a compliment based on upstream greetings"""
    # Get the first upstream greeting
    greeting_message = "Hello!"
    if upstream:
        for task_output in upstream.values():
            if "message" in task_output:
                greeting_message = task_output["message"]
                break

    # Simulate work
    time.sleep(5)

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result
