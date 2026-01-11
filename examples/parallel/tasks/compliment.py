"""Ork task wrapper for compliments"""
from pydantic import BaseModel
from src.greetings import create_compliment


class GreetOutput(BaseModel):
    message: str
    timestamp: float


class ComplimentInput(BaseModel):
    greet: GreetOutput
    adjective: str = "fast"


class ComplimentOutput(BaseModel):
    line: str
    adjective: str


def main(input: ComplimentInput) -> ComplimentOutput:
    result = create_compliment(input.greet.message, input.adjective)
    print(result["line"])
    return ComplimentOutput(line=result["line"], adjective=result["adjective"])
