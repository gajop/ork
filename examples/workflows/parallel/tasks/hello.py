"""Ork task wrapper for greeting"""
from pydantic import BaseModel
from src.greetings import create_greeting


class HelloInput(BaseModel):
    name: str = "world"
    delay: float = 1.0


class HelloOutput(BaseModel):
    message: str
    timestamp: float


def main(input: HelloInput) -> HelloOutput:
    result = create_greeting(input.name, input.delay)
    print(result["message"])
    return HelloOutput(message=result["message"], timestamp=result["timestamp"])
