"""Python tasks for polyglot workflow example."""
import json
import sys


def generate_numbers(count: int) -> dict:
    """Generate a list of numbers (Python task 1)."""
    print(f"[Python] Generating {count} numbers...")
    numbers = list(range(1, count + 1))
    print(f"[Python] Generated: {numbers}")
    return {"numbers": numbers}


def format_result(multiplier: int) -> dict:
    """Format the final result (Python task 3)."""
    import os
    import json

    print(f"[Python] Formatting result with multiplier {multiplier}...")

    # Get upstream data from environment
    upstream_json = os.getenv("ORK_UPSTREAM_JSON", "{}")
    upstream = json.loads(upstream_json)

    print(f"[Python] Received upstream data: {upstream}")

    # Extract rust_process output
    processed = upstream.get("rust_process", {})
    total = processed.get("sum", 0)
    doubled = processed.get("doubled", [])

    result = {
        "summary": f"Processed {len(doubled)} numbers",
        "total": total,
        "final_value": total * multiplier,
        "doubled_numbers": doubled
    }

    print(f"[Python] Final result: {result}")
    return result
