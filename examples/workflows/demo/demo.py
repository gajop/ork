"""Demo workflow - TypedDict example"""
import time
from datetime import datetime
from typing import TypedDict


class GreetOutput(TypedDict):
    message: str
    timestamp: datetime


class ComplimentOutput(TypedDict):
    line: str
    adjective: str


class ShellEchoOutput(TypedDict):
    note: str
    finished_at: datetime


def greet(name: str = "world", delay: float = 1.0) -> GreetOutput:
    """Generate a greeting message."""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: GreetOutput = {"message": message, "timestamp": datetime.now()}
    print(result["message"])
    return result


def compliment(greet: GreetOutput, adjective: str = "fast") -> ComplimentOutput:
    """Create a compliment based on greeting output."""
    greeting_message = greet.get("message", "Hello!")

    time.sleep(5)

    line = f"{greeting_message} - you built Ork to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result


def shell_echo(
    compliment: ComplimentOutput,
    delay: float = 5.0,
    note: str = "python fallback for shell commands",
) -> ShellEchoOutput:
    """Process a note with some delay."""
    time.sleep(delay)
    final_note = f"{note}: {compliment['line']}"
    result: ShellEchoOutput = {"note": final_note, "finished_at": datetime.now()}
    print(result["note"])
    return result
