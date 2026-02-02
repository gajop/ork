"""
Proposed Ork Implementation - Pure Python
Same as examples/workflows/demo - greeting followed by compliment

This shows a PROPOSED approach using functional @task decorator.

Installation:
  cargo build --release

Run:
  ork run 9_ork_proposed_python.py
  # OR just: python 9_ork_proposed_python.py
"""

from ork import task, workflow
import time

@task
def greet(name: str = "Ork dev", delay: float = 5.0) -> dict:
    """Generate a greeting message"""
    time.sleep(delay)
    message = f"Hello, {name}!"
    print(message)
    return {"message": message, "timestamp": time.time()}

@task
def compliment(greet: dict, adjective: str = "ridiculously fast") -> dict:
    """Create a compliment based on a greeting"""
    time.sleep(5)
    line = f"{greet['message']} — you built Ork to be {adjective}!"
    print(line)
    return {"line": line, "adjective": adjective}

@task
def shell_echo(compliment: dict) -> None:
    """Echo the compliment"""
    print(f"Final: {compliment['line']}")

@workflow(name="demo")
def demo_workflow():
    """Define the workflow"""
    g = greet()
    c = compliment(g)
    shell_echo(c)

if __name__ == "__main__":
    demo_workflow.run()

"""
TOTAL: 1 file, ~30 lines

PROS:
+ Single file (vs 4 files)
+ No Pydantic boilerplate (vs 40+ lines)
+ No schema duplication
+ Natural Python (feels like normal functions)
+ @task actually does something
+ Can still run without server
+ Easy to test (just Python functions)
+ Low barrier to entry

CONS:
- Locks you into Python (can't easily mix shell/dbt/etc in same file)
- Less declarative than YAML (harder to visualize DAG)
- Type safety is weaker (dict vs Pydantic models)

DESIGN QUESTIONS:
1. How to handle different executors? (Python, shell, dbt, etc)
   - Could have @task(executor="dbt") but feels clunky
   - Or keep YAML for multi-executor workflows

2. Should we auto-generate types from annotations?
   - @task could introspect function signature
   - Generate Input/Output models automatically
   - Best of both worlds?

3. How to pass parameters?
   - Option A: demo_workflow.run(param1="value")
   - Option B: demo_workflow(param1="value").run()
   - Option C: CLI args: ork run script.py --param1=value
"""
