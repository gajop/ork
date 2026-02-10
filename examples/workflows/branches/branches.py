"""Branches workflow - consolidated single-file example"""
import time
from typing import Dict, Optional
from pydantic import BaseModel


# Business logic functions
def create_greeting(name: str, delay: float = 0) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    return {"message": message, "timestamp": time.time()}


def create_compliment(greeting_message: str, adjective: str) -> dict:
    """Create a compliment based on a greeting"""
    time.sleep(5)  # Simulate work

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    return {"line": line, "adjective": adjective}


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


# Compliment task
class GreetOutput(BaseModel):
    message: str
    timestamp: float


class ComplimentInput(BaseModel):
    greet: Optional[GreetOutput] = None
    upstream: Optional[Dict[str, GreetOutput]] = None
    adjective: str = "fast"


class ComplimentOutput(BaseModel):
    line: str
    adjective: str


def _resolve_greet(input: ComplimentInput) -> GreetOutput:
    if input.greet:
        return input.greet
    if input.upstream:
        if "greet" in input.upstream:
            return input.upstream["greet"]
        return next(iter(input.upstream.values()))
    raise ValueError("missing greet output")


def compliment(input: ComplimentInput) -> ComplimentOutput:
    greet_output = _resolve_greet(input)
    result = create_compliment(greet_output.message, input.adjective)
    print(result["line"])
    return ComplimentOutput(line=result["line"], adjective=result["adjective"])
