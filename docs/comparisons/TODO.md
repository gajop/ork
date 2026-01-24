# Framework Testing Progress

## Python Orchestrators

| Framework | Server | parallel_tasks | data_pipeline | conditional_branching | Persists |
|-----------|--------|----------------|---------------|----------------------|----------|
| airflow | ✅ | ✅ | ✅ | ✅ | ✅ |
| prefect | ✅ | ✅ | ✅ | ✅ | ✅ |
| dagster | ✅ | ✅ | ✅ | ✅ | ✅ |
| luigi | ✅ | ✅ | ✅ | ✅ | ✅ |
| mage | ✅ | ✅ | ✅ | ✅ | ✅ |
| kedro | ✅ (viz only) | ✅ | ✅ | ✅ | ✅ |
| metaflow | N/A (cloud-focused) | ✅ | ✅ | ✅ | ✅ |

## YAML Orchestrators

| Framework | Server | parallel_tasks | data_pipeline | conditional_branching | Persists |
|-----------|--------|----------------|---------------|----------------------|----------|
| kestra | ✅ | ✅ | ✅ | ✅ | ✅ |

## Notes
- **kedro**: Visualization only (kedro-viz), no run triggering from UI
- **metaflow**: Designed for cloud (AWS/K8s), local UI requires deploying metaflow-ui service
- **kestra**: YAML-based, runs in Docker, supports namespace files for external Python scripts, uses `Parallel` and `If` tasks for flow control

## Cloud/CI Orchestrators (later)

| Framework | Notes |
|-----------|-------|
| github-actions | |
| gitlab-ci | |
| aws-step-functions | |
| azure-data-factory | |
| databricks-workflows | |
| gcp-workflows | |
