"""Demo workflow - plain Python functions example"""
import time


def greet(name: str = "world", delay: float = 1.0) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result = {"message": message, "timestamp": time.time()}
    print(result["message"])
    return result


def compliment(adjective: str = "fast", upstream: dict = None) -> dict:
    """Create a compliment based on upstream greeting"""
    # Get the greeting from upstream
    greet_output = upstream.get("greet", {}) if upstream else {}
    greeting_message = greet_output.get("message", "Hello!")

    # Simulate work
    time.sleep(5)

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    result = {"line": line, "adjective": adjective}
    print(result["line"])
    return result


def shell_echo(delay: float = 5.0, note: str = "python fallback for shell commands", upstream: dict = None) -> dict:
    """Process a note with some delay"""
    time.sleep(delay)
    result = {"note": note, "finished_at": time.time()}
    print(result["note"])
    return result
