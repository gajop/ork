"""Ork task wrapper for note processing"""
from pydantic import BaseModel
from src.greetings import process_note


class ShellEchoInput(BaseModel):
    delay: float = 5.0
    note: str = "python fallback for shell commands"


class ShellEchoOutput(BaseModel):
    note: str
    finished_at: float


def main(input: ShellEchoInput) -> ShellEchoOutput:
    result = process_note(input.note, input.delay)
    print(result["note"])
    return ShellEchoOutput(note=result["note"], finished_at=result["finished_at"])
