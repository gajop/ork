"""
Proposed Ork Implementation - Hybrid Approach
Same as examples/demo - greeting followed by compliment

This shows a PROPOSED hybrid: Python tasks with optional Pydantic.

Key idea: Make Pydantic OPTIONAL, use type hints to auto-generate schemas.

Installation:
  cargo build --release

Run:
  ork run 11_ork_proposed_hybrid.py
"""

from ork import task, workflow
from typing import TypedDict  # Standard library, not Pydantic
import time

# Option 1: Use plain dicts (simplest)
@task
def greet_simple(name: str = "Ork dev", delay: float = 5.0) -> dict:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

# Option 2: Use TypedDict (better IDE support, no Pydantic)
class GreetOutput(TypedDict):
    message: str
    timestamp: float

@task
def greet_typed(name: str = "Ork dev", delay: float = 5.0) -> GreetOutput:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

# Option 3: Use Pydantic (maximum type safety)
from pydantic import BaseModel

class GreetOutputStrict(BaseModel):
    message: str
    timestamp: float

@task
def greet_pydantic(name: str = "Ork dev", delay: float = 5.0) -> GreetOutputStrict:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return GreetOutputStrict(message=message, timestamp=time.time())

# Downstream task - receives upstream output
@task
def compliment(greet: dict, adjective: str = "ridiculously fast") -> dict:
    """
    Create a compliment based on a greeting

    The @task decorator auto-generates Input schema from:
    - Function parameters (greet: dict, adjective: str)
    - Default values
    - Type hints

    No need to define ComplimentInput class!
    """
    time.sleep(5)
    line = f"{greet['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@workflow(name="demo")
def demo_workflow():
    """Define the workflow"""
    # Choose your preferred style
    g = greet_simple()  # or greet_typed() or greet_pydantic()
    c = compliment(g, adjective="ridiculously fast")

if __name__ == "__main__":
    demo_workflow.run()

"""
HOW IT WORKS:

1. @task decorator introspects function signature:
   def greet(name: str = "Ork dev", delay: float = 5.0) -> dict:

2. Auto-generates Input schema:
   class GreetInput(BaseModel):
       name: str = "Ork dev"
       delay: float = 5.0

3. Auto-generates Output schema from return type:
   - dict -> Any (permissive)
   - TypedDict -> Pydantic model matching TypedDict
   - BaseModel -> use as-is

4. At runtime:
   - Validates inputs using generated schema
   - Runs function
   - Validates outputs using generated schema
   - Passes to downstream tasks

BENEFITS:
+ No manual Input/Output class definitions (30+ lines saved per task)
+ No schema duplication (downstream tasks use dict)
+ Type hints provide IDE support
+ Can opt-in to Pydantic for strict validation
+ Backwards compatible (can still use full Pydantic)

TOTAL: 1 file, ~40 lines (with all 3 examples)
       1 file, ~15 lines (with just greet_simple)

PROS:
+ Drastically less verbose than current Ork
+ No schema duplication
+ Flexible (dict/TypedDict/Pydantic)
+ Natural Python
+ Easy to test
+ Low barrier to entry

CONS:
- Still Python-only in this file
- Runtime schema generation has small overhead
- Less explicit than current approach
"""
