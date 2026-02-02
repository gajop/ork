"""Business logic - no Ork dependencies"""
import time


def create_greeting(name: str, delay: float = 0) -> dict:
    """Generate a greeting message"""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    return {"message": message, "timestamp": time.time()}


def create_compliment(greeting_message: str, adjective: str) -> dict:
    """Create a compliment based on a greeting"""
    time.sleep(5)  # Simulate work

    line = f"{greeting_message} — you built Ork to be {adjective}!"
    return {"line": line, "adjective": adjective}


def process_note(note: str, delay: float = 5.0) -> dict:
    """Process a note with some delay"""
    time.sleep(delay)
    return {"note": note, "finished_at": time.time()}
