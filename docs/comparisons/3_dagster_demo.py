"""
Dagster Implementation of Demo Workflow
Same as examples/demo - greeting followed by compliment

Note: Dagster is asset-based (data-centric) rather than task-based,
so this example shows both approaches.

Installation:
  pip install dagster dagster-webserver

Run:
  dagster dev -f 3_dagster_demo.py
  # Opens UI at http://localhost:3000
"""

from dagster import asset, op, job, Definitions, In, Out
import time

# Approach 1: Asset-based (Dagster's preferred way)
# Assets represent data/artifacts that are produced

@asset
def greeting_message() -> dict:
    """Generate a greeting message (as an asset)"""
    name = "Ork dev"
    delay = 5
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@asset
def compliment_line(greeting_message: dict) -> dict:
    """Create a compliment based on greeting (as an asset)"""
    adjective = "ridiculously fast"
    time.sleep(5)
    line = f"{greeting_message['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@asset
def final_output(compliment_line: dict) -> str:
    """Echo the compliment (as an asset)"""
    print(f"Final: {compliment_line['line']}")
    return compliment_line['line']

# Approach 2: Op/Job-based (more like traditional workflows)
# Ops are tasks, jobs are workflows

@op(out=Out(dict))
def greet_op():
    """Generate a greeting message"""
    name = "Ork dev"
    delay = 5
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@op(ins={"greet_output": In(dict)}, out=Out(dict))
def compliment_op(greet_output):
    """Create a compliment based on a greeting"""
    adjective = "ridiculously fast"
    time.sleep(5)
    line = f"{greet_output['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@op(ins={"compliment_output": In(dict)})
def shell_echo_op(compliment_output):
    """Echo the compliment"""
    print(f"Final: {compliment_output['line']}")

@job
def demo_workflow_job():
    """Define the workflow using ops"""
    greet_result = greet_op()
    compliment_result = compliment_op(greet_result)
    shell_echo_op(compliment_result)

# Dagster requires a Definitions object
defs = Definitions(
    assets=[greeting_message, compliment_line, final_output],
    jobs=[demo_workflow_job],
)

"""
PROS:
+ Asset-centric (great for data pipelines)
+ Excellent UI and observability
+ Strong typing with type checking
+ Great testing framework
+ Powerful versioning and lineage
+ Can run assets in different environments

CONS:
- Requires server (dagster dev)
- Conceptually more complex (assets vs ops vs jobs)
- Overkill for simple task orchestration
- ~55 lines for simple workflow
- Asset model doesn't fit all use cases (more for data than tasks)
- Heavy (brings its own web framework)
"""
