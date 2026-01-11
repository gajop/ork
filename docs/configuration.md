Status: Pending Review

# Configuration

## Retries

Configure automatic retries for transient failures:

```yaml
tasks:
  flaky_task:
    executor: python
    file: tasks/flaky.py
    retries: 3
    retry_delay: 60  # seconds
```

- `retries`: Maximum retry attempts (default: 0)
- `retry_delay`: Seconds to wait between retries (default: 0)

### Retry Behavior

1. Task fails (non-zero exit or timeout)
2. If `attempt < max_retries`, task transitions back to `pending`
3. After `retry_delay` seconds, scheduler dispatches task again
4. If all retries exhausted, task marked as `failed`

### Exponential Backoff

Not currently supported. Use a custom executor if needed:

```py
@executor
class RetryExecutor:
    def run(self, config_file: str, ctx: Context, upstream: dict = {}) -> TaskOutput:
        for attempt in range(self.max_retries):
            try:
                return self.execute(config_file, ctx, upstream)
            except Exception as e:
                if attempt < self.max_retries - 1:
                    delay = self.initial_delay * (2 ** attempt)
                    time.sleep(delay)
                else:
                    raise
```

## Resources

Specify CPU and memory requirements:

```yaml
tasks:
  heavy_task:
    executor: python
    file: tasks/heavy.py
    timeout: 3600
    resources:
      cpu: 4      # cores or millicores
      memory: 8Gi # or Mi, Gi
```

### CPU

- Integer values: cores (e.g., `4` = 4 cores)
- Millicores: `1000` = 1 core

### Memory

- Units: `Mi` (mebibytes), `Gi` (gibibytes)
- Examples: `512Mi`, `4Gi`, `16Gi`

### Timeouts

- `timeout`: Maximum execution time in seconds (default: 3600)
- Task killed if execution exceeds timeout

## Environment Variables

### Static Values

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    env:
      LOG_LEVEL: debug
      API_URL: https://api.example.com
      MAX_RETRIES: "3"
```

Environment variables are available to task code:

```py
import os

@task
def main(input: Input) -> Output:
    log_level = os.getenv("LOG_LEVEL", "info")
    api_url = os.getenv("API_URL")
    # ...
```

### References

Reference values from upstream tasks:

```yaml
tasks:
  discover:
    executor: python
    file: tasks/discover.py

  process:
    executor: python
    file: tasks/process.py
    depends_on: [discover]
    env:
      DATA_PATH: "{{ discover.output_path }}"
```

## Secrets

### Secret Manager

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    env:
      API_KEY:
        secret: projects/my-project/secrets/api-key/versions/latest
      DB_PASSWORD:
        secret: projects/my-project/secrets/db-password/versions/latest
```

Ork fetches secrets at task dispatch time from the configured secret manager:
- GCP: Secret Manager
- AWS: Secrets Manager
- Azure: Key Vault

### File Secrets

Mount secrets as files:

```yaml
tasks:
  extract:
    executor: python
    file: tasks/extract.py
    secrets:
      - name: service-account
        secret: projects/my-project/secrets/sa-key
        path: /secrets/sa-key.json
```

Access in task:

```py
import json

@task
def main(input: Input) -> Output:
    with open("/secrets/sa-key.json") as f:
        credentials = json.load(f)
    # ...
```

### Local Development

For local development, use a `.env` file:

```bash
# .env
API_KEY=dev-key-123
DB_PASSWORD=dev-password
```

```yaml
# ork.yaml (dev config)
env_file: .env
```

Or export environment variables:

```bash
export API_KEY=dev-key-123
export DB_PASSWORD=dev-password
ork run start my_etl
```

## Project Configuration

```yaml
# ork.yaml
executors:
  python:
    type: python
  rust:
    type: rust
  bigquery:
    type: executors.bigquery:BigQueryExecutor
    config:
      project: my-gcp-project

# Development
env_file: .env

# API settings
api:
  url: http://localhost:8080
  auth:
    type: api_key
    header: X-API-Key
    value:
      secret: projects/my-project/secrets/api-key
```
