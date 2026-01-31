Status: Pending Review

# Tasks

A task wraps existing code and defines inputs and outputs using Pydantic models (Python) or serde (Rust).

## Python Tasks

### Existing Code

```py
# src/extract.py
import requests
from pydantic import BaseModel

class User(BaseModel):
    id: int
    name: str
    email: str

def fetch_users(api_url: str) -> list[User]:
    response = requests.get(f"{api_url}/users")
    return [User(**u) for u in response.json()]
```

### Task Wrapper

```py
# tasks/extract.py
from ork import task
from pydantic import BaseModel
from src.extract import fetch_users, User

class Input(BaseModel):
    api_url: str = "https://api.example.com"

class Output(BaseModel):
    users: list[User]

@task
def main(input: Input) -> Output:
    users = fetch_users(input.api_url)
    return Output(users=users)
```

## Upstream Dependencies

Downstream tasks receive upstream outputs. The field name matches the upstream task name:

```py
# tasks/transform.py
from ork import task
from pydantic import BaseModel
from src.extract import User
from src.transform import enrich, EnrichedUser

class ExtractOutput(BaseModel):
    users: list[User]

class Input(BaseModel):
    extract: ExtractOutput

class Output(BaseModel):
    users: list[EnrichedUser]

@task
def main(input: Input) -> Output:
    enriched = enrich(input.extract.users)
    return Output(users=enriched)
```

## Rust Tasks

```rust
// tasks/extract.rs
use ork::task;
use serde::{Deserialize, Serialize};
use crate::extract::fetch_users;

#[derive(Deserialize)]
struct Input {
    #[serde(default = "default_api_url")]
    api_url: String,
}

fn default_api_url() -> String {
    "https://api.example.com".into()
}

#[derive(Serialize, Deserialize)]
struct User {
    id: i64,
    name: String,
    email: String,
}

#[derive(Serialize)]
struct Output {
    users: Vec<User>,
}

#[task]
fn main(input: Input) -> ork::Result<Output> {
    let users = fetch_users(&input.api_url)?;
    Ok(Output { users })
}
```

## Other Languages

For languages without SDK support, tasks read JSON from stdin and write JSON to stdout:

```bash
#!/bin/bash
# tasks/process.sh

# Read input
INPUT=$(cat)
FILE=$(echo $INPUT | jq -r '.file')

# Process
LINES=$(wc -l < "$FILE")

# Write output
echo "{\"lines\": $LINES}"
exit 0
```

Exit code 0 indicates success, non-zero indicates failure.
