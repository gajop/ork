"""Python tasks for polyglot workflow example - plain functions"""


def generate_numbers(count: int) -> dict:
    """Generate a list of numbers (Python task 1)."""
    print(f"[Python] Generating {count} numbers...")
    numbers = list(range(1, count + 1))
    print(f"[Python] Generated: {numbers}")
    return {"numbers": numbers}


def format_result(multiplier: int, upstream: dict = None) -> dict:
    """Format the final result (Python task 3)."""
    print(f"[Python] Formatting result with multiplier {multiplier}...")
    print(f"[Python] Received upstream data: {upstream}")

    # Extract rust_process_sdk output
    processed = upstream.get("rust_process_sdk", {}) if upstream else {}
    total = processed.get("total", 0)
    tripled = processed.get("tripled", [])

    result = {
        "summary": f"Processed {len(tripled)} numbers",
        "total": total,
        "final_value": total * multiplier,
        "tripled_numbers": tripled
    }

    print(f"[Python] Final result: {result}")
    return result
