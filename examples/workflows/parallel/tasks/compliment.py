"""Ork task wrapper for compliments"""
from typing import Dict, Optional

from pydantic import BaseModel
from src.greetings import create_compliment


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


def main(input: ComplimentInput) -> ComplimentOutput:
    greet = _resolve_greet(input)
    result = create_compliment(greet.message, input.adjective)
    print(result["line"])
    return ComplimentOutput(line=result["line"], adjective=result["adjective"])
