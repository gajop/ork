"""
Temporal Implementation of Demo Workflow
Same as examples/demo - greeting followed by compliment

Installation:
  pip install temporalio

Run:
  # Start Temporal server (requires Docker):
  docker run -p 7233:7233 -p 8233:8233 temporalio/auto-setup:latest

  # In another terminal, run worker:
  python 4_temporal_demo.py worker

  # In another terminal, run workflow:
  python 4_temporal_demo.py run
"""

from temporalio import activity, workflow
from temporalio.client import Client
from temporalio.worker import Worker
from datetime import timedelta
import asyncio
import time
import sys

@activity.defn
async def greet() -> dict:
    """Generate a greeting message"""
    name = "Ork dev"
    delay = 5
    time.sleep(delay)  # Blocking sleep in activity is OK
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@activity.defn
async def compliment(greet_output: dict) -> dict:
    """Create a compliment based on a greeting"""
    adjective = "ridiculously fast"
    time.sleep(5)
    line = f"{greet_output['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@activity.defn
async def shell_echo(compliment_output: dict) -> None:
    """Echo the compliment"""
    print(f"Final: {compliment_output['line']}")

@workflow.defn
class DemoWorkflow:
    """Demo workflow definition"""

    @workflow.run
    async def run(self) -> dict:
        """Execute the workflow"""
        # Execute activities with timeout
        greet_result = await workflow.execute_activity(
            greet,
            start_to_close_timeout=timedelta(seconds=30),
        )

        compliment_result = await workflow.execute_activity(
            compliment,
            greet_result,
            start_to_close_timeout=timedelta(seconds=30),
        )

        await workflow.execute_activity(
            shell_echo,
            compliment_result,
            start_to_close_timeout=timedelta(seconds=30),
        )

        return compliment_result

async def run_worker():
    """Run a worker that executes activities"""
    client = await Client.connect("localhost:7233")
    worker = Worker(
        client,
        task_queue="demo-task-queue",
        workflows=[DemoWorkflow],
        activities=[greet, compliment, shell_echo],
    )
    await worker.run()

async def run_workflow():
    """Execute the workflow"""
    client = await Client.connect("localhost:7233")
    result = await client.execute_workflow(
        DemoWorkflow.run,
        id="demo-workflow-1",
        task_queue="demo-task-queue",
    )
    print(f"Workflow completed: {result}")

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "worker":
        asyncio.run(run_worker())
    else:
        asyncio.run(run_workflow())

"""
PROS:
+ Extremely durable execution (survives crashes, restarts)
+ Built-in retries, timeouts, compensation
+ Event sourcing (full workflow history)
+ Scales to massive throughput
+ Strong consistency guarantees
+ Can pause/resume workflows for days/months

CONS:
- Very complex (activities, workflows, workers, async everywhere)
- Requires Temporal server (heavy infrastructure)
- Overkill for batch jobs
- ~80 lines for simple workflow
- Steep learning curve
- Async/await everywhere adds complexity
- Need to run worker processes separately
- More suited for long-running business processes than data pipelines
"""
