"""Python tasks for polyglot workflow example - TypedDict."""
from typing import TypedDict


class NumbersOutput(TypedDict):
    numbers: list[int]


class RustLibraryOutput(TypedDict):
    tripled: list[int]
    sum: int


class RustProcessSdkOutput(TypedDict):
    total: int
    tripled: list[int]


class FormatOutput(TypedDict):
    summary: str
    total: int
    final_value: int
    tripled_numbers: list[int]


def generate_numbers(count: int) -> NumbersOutput:
    """Generate a list of numbers (Python task 1)."""
    print(f"[Python] Generating {count} numbers...")
    numbers = list(range(1, count + 1))
    print(f"[Python] Generated: {numbers}")
    return {"numbers": numbers}


def format_result(multiplier: int, rust_process_sdk: RustProcessSdkOutput) -> FormatOutput:
    """Format the final result (Python task 3)."""
    print(f"[Python] Formatting result with multiplier {multiplier}...")
    print(f"[Python] Received rust_process_sdk data: {rust_process_sdk}")

    total = rust_process_sdk.get("total", 0)
    tripled = rust_process_sdk.get("tripled", [])

    result: FormatOutput = {
        "summary": f"Processed {len(tripled)} numbers",
        "total": total,
        "final_value": total * multiplier,
        "tripled_numbers": tripled,
    }

    print(f"[Python] Final result: {result}")
    return result
