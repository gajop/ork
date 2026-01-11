"""
Prefect Implementation of Demo Workflow
Same as examples/demo - greeting followed by compliment

Installation:
  pip install prefect

Run:
  python 2_prefect_demo.py

  # For UI/scheduling (optional):
  prefect server start
  python 2_prefect_demo.py
"""

from prefect import flow, task
import time

@task
def greet(name: str = "Ork dev", delay: float = 5.0) -> dict:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@task
def compliment(greet_output: dict, adjective: str = "ridiculously fast") -> dict:
    """Create a compliment based on a greeting"""
    time.sleep(5)
    line = f"{greet_output['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@task
def shell_echo(compliment_output: dict) -> None:
    """Echo the compliment"""
    print(f"Final: {compliment_output['line']}")

@flow(name="demo_workflow")
def demo_workflow():
    """Main workflow orchestrating the tasks"""
    greet_result = greet()
    compliment_result = compliment(greet_result)
    shell_echo(compliment_result)

if __name__ == "__main__":
    demo_workflow()

"""
PROS:
+ Clean Pythonic API (@task/@flow decorators)
+ Runs locally without server (server optional)
+ Good type hints and IDE support
+ Easy to test (just Python functions)
+ Automatic retries and caching
+ Modern UI (when using Prefect Cloud/Server)

CONS:
- Needs Prefect Cloud or self-hosted server for scheduling
- Some features require paid Prefect Cloud
- Still ~35 lines for simple workflow
- Limited to Python (can call subprocesses but not first-class)
"""
