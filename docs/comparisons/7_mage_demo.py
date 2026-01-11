"""
Mage Implementation of Demo Workflow
Same as examples/demo - greeting followed by compliment

Mage is a hybrid tool - modern UI like Airflow, but with better ergonomics.
It uses @data_loader, @transformer, @data_exporter decorators.

Installation:
  pip install mage-ai

Run:
  mage start demo_project
  # Opens UI at http://localhost:6789
  # Create a new pipeline in the UI and paste blocks below
"""

import time

# Block 1: Data Loader (equivalent to 'greet' task)
@data_loader
def greet(*args, **kwargs) -> dict:
    """
    Generate a greeting message
    """
    name = "Ork dev"
    delay = 5
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

# Block 2: Transformer (equivalent to 'compliment' task)
@transformer
def compliment(greet_output: dict, *args, **kwargs) -> dict:
    """
    Create a compliment based on a greeting

    Args:
        greet_output: Output from greet block
    """
    adjective = "ridiculously fast"
    time.sleep(5)
    line = f"{greet_output['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

# Block 3: Data Exporter (equivalent to 'shell_echo' task)
@data_exporter
def shell_echo(compliment_output: dict, *args, **kwargs) -> None:
    """
    Echo the compliment

    Args:
        compliment_output: Output from compliment block
    """
    print(f"Final: {compliment_output['line']}")

"""
Alternative: Mage also supports pipelines as code (without UI)

# pipeline.py
from mage_ai.data_preparation.decorators import pipeline

@pipeline
def demo_workflow():
    greet_result = greet()
    compliment_result = compliment(greet_result)
    shell_echo(compliment_result)
"""

"""
PROS:
+ Modern UI with notebook-like interface
+ Good for data engineering (built-in connectors)
+ Decorators are clean (@data_loader, @transformer, @data_exporter)
+ Can develop in UI or code
+ Built-in data quality checks
+ Good visualization of data flow
+ Git integration
+ DBT integration

CONS:
- Requires server (Mage daemon)
- UI-first workflow (harder to do pure code)
- Prescriptive block types (loader/transformer/exporter)
- ~35 lines for simple workflow
- Smaller community than Airflow
- Less flexible than pure-Python tools
- Data-engineering focused (not general purpose)
"""
