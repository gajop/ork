"""Ork task wrapper for flakey operation"""
from pydantic import BaseModel
from src.retry_logic import attempt_operation


class FlakeyInput(BaseModel):
    fail_times: int = 0


class FlakeyOutput(BaseModel):
    ok: bool
    attempt: int


def main(input: FlakeyInput) -> FlakeyOutput:
    result = attempt_operation(input.fail_times)
    print(f"succeeded on attempt {result['attempt']}")
    return FlakeyOutput(ok=result["ok"], attempt=result["attempt"])
