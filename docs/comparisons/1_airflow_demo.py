"""
Airflow Implementation of Demo Workflow (Modern @task API)
Same as examples/demo - greeting followed by compliment

Installation:
  pip install apache-airflow

Run:
  # Requires Airflow server running
  airflow db init
  airflow webserver -p 8080
  airflow scheduler
  # Then trigger via UI or: airflow dags trigger demo_workflow
"""

from airflow.decorators import dag, task
from datetime import datetime
import time

@dag(
    dag_id='demo_workflow',
    start_date=datetime(2024, 1, 1),
    schedule=None,  # Manual trigger only
    catchup=False,
    tags=['demo'],
)
def demo_workflow():
    """Demo greeting workflow"""

    @task
    def greet() -> dict:
        """Generate a greeting message"""
        name = "Ork dev"
        delay = 5
        time.sleep(delay)
        message = f"Hello, {name}!"
        print(message)
        return {"message": message, "timestamp": time.time()}

    @task
    def compliment(greet_output: dict) -> dict:
        """Create a compliment based on a greeting"""
        message = greet_output['message']
        adjective = "ridiculously fast"
        time.sleep(5)
        line = f"{message} — you built Ork to be {adjective}!"
        print(line)
        return {"line": line, "adjective": adjective}

    @task
    def shell_echo(compliment_output: dict) -> None:
        """Echo the compliment"""
        print(f"Final: {compliment_output['line']}")

    # Define workflow
    greet_result = greet()
    compliment_result = compliment(greet_result)
    shell_echo(compliment_result)

# Instantiate the DAG
demo_workflow()

"""
PROS:
+ Mature ecosystem with tons of operators
+ Powerful scheduling (cron, sensors, etc)
+ Great UI with task logs, retries, etc
+ Battle-tested in production
+ Modern @task API is much cleaner (40 lines vs 60+)
+ TaskFlow API handles XCom automatically

CONS:
- Requires server (webserver + scheduler + database)
- Still ~45 lines for simple workflow
- Hard to test locally (needs DB)
- Heavy dependencies
- DAG file must be in specific directory
- Nested function definitions feel awkward
"""
