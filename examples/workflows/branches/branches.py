"""Branches workflow - plain Python functions example"""
import time


def hello(name: str = "world", delay: float = 1.0, upstream: dict = None) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result = {"message": message, "timestamp": time.time()}
    print(result["message"])
    return result


def compliment(adjective: str = "fast", upstream: dict = None) -> dict:
    """Create a compliment based on upstream greetings"""
    # Get the first upstream greeting (could be from left or right branch)
    greeting_message = "Hello!"
    if upstream:
        for task_output in upstream.values():
            if "message" in task_output:
                greeting_message = task_output["message"]
                break

    # Simulate work
    time.sleep(5)

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    result = {"line": line, "adjective": adjective}
    print(result["line"])
    return result
