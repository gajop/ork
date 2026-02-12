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
    """Generate a greeting message."""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: HelloOutput = {"message": message, "timestamp": datetime.now()}
    print(result["message"])
    return result


def compliment(
    hello_left: HelloOutput,
    hello_right: HelloOutput,
    adjective: str = "fast",
) -> ComplimentOutput:
    """Create a compliment based on both upstream greetings."""
    greeting_message = hello_left.get("message") or hello_right.get("message") or "Hello!"

    time.sleep(5)

    line = f"{greeting_message} - you built Ork to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result
