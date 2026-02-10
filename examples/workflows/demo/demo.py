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
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: GreetOutput = {"message": message, "timestamp": datetime.now()}
    print(result["message"])
    return result


def compliment(adjective: str = "fast", upstream: dict = None) -> ComplimentOutput:
    """Create a compliment based on upstream greeting"""
    # Get the greeting from upstream
    greet_output = upstream.get("greet", {}) if upstream else {}
    greeting_message = greet_output.get("message", "Hello!")

    # Simulate work
    time.sleep(5)

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result


def shell_echo(delay: float = 5.0, note: str = "python fallback for shell commands", upstream: dict = None) -> ShellEchoOutput:
    """Process a note with some delay"""
    time.sleep(delay)
    result: ShellEchoOutput = {"note": note, "finished_at": datetime.now()}
    print(result["note"])
    return result
