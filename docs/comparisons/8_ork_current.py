"""
Current Ork Implementation of Demo Workflow
Same as examples/demo - greeting followed by compliment

This shows the CURRENT state with typed Pydantic approach.

Installation:
  cargo build --release
  # Copy binary to PATH or use cargo run

Run:
  ork run 8_ork_current.yaml

Structure:
  8_ork_current.yaml          # Workflow definition
  tasks/
    hello.py                   # Task 1
    compliment.py              # Task 2
    shell_echo.py              # Task 3
  src/
    greetings.py               # Business logic (optional)
"""

# File: tasks/hello.py
"""
from pydantic import BaseModel
import time

class HelloInput(BaseModel):
    name: str = "world"
    delay: float = 1.0

class HelloOutput(BaseModel):
    message: str
    timestamp: float

def main(input: HelloInput) -> HelloOutput:
    time.sleep(input.delay)
    message = f"Hello, {input.name}!"
    print(message)
    return HelloOutput(message=message, timestamp=time.time())
"""

# File: tasks/compliment.py
"""
from pydantic import BaseModel
import time

# Must redefine upstream output schema!
class GreetOutput(BaseModel):
    message: str
    timestamp: float

class ComplimentInput(BaseModel):
    greet: GreetOutput  # Receives upstream output
    adjective: str = "fast"

class ComplimentOutput(BaseModel):
    line: str
    adjective: str

def main(input: ComplimentInput) -> ComplimentOutput:
    time.sleep(5)
    line = f"{input.greet.message} — you built Ork to be {input.adjective}!"
    print(line)
    return ComplimentOutput(line=line, adjective=input.adjective)
"""

# File: tasks/shell_echo.py
"""
from pydantic import BaseModel

class ComplimentOutput(BaseModel):  # Must redefine again!
    line: str
    adjective: str

class ShellEchoInput(BaseModel):
    compliment: ComplimentOutput

class ShellEchoOutput(BaseModel):
    pass  # No output needed

def main(input: ShellEchoInput) -> ShellEchoOutput:
    print(f"Final: {input.compliment.line}")
    return ShellEchoOutput()
"""

# File: 8_ork_current.yaml
"""
name: local_demo

tasks:
  greet:
    executor: python
    file: tasks/hello.py
    input:
      name: "Ork dev"
      delay: 5

  compliment:
    executor: python
    file: tasks/compliment.py
    depends_on: [greet]
    input:
      adjective: "ridiculously fast"

  shell_echo:
    executor: python
    file: tasks/shell_echo.py
    depends_on: [compliment]
"""

"""
TOTAL: 4 files, ~70 lines of boilerplate

PROS:
+ No server required (truly serverless)
+ Strong type safety with Pydantic
+ Runs locally with simple CLI
+ Multi-language (Python, shell, future: dbt, SQL, etc)
+ Clean separation of workflow (YAML) and code (Python)
+ Low/no cost at idle

CONS:
- VERY verbose (70+ lines for 3 simple tasks)
- Schema duplication (downstream tasks must redefine upstream outputs)
- Generic class names (Input/Output everywhere)
- @task decorator does nothing (just documentation)
- Too many files for simple workflows
- High barrier to entry
- Pydantic boilerplate for every task
"""
