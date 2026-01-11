# Authoritative Example Links for Each Framework

## GitHub Repositories & Popularity

| Tool | GitHub | Stars | Description |
|------|--------|-------|-------------|
| **Airflow** | [apache/airflow](https://github.com/apache/airflow) | ⭐ 43.8k | Apache Foundation, most mature |
| **Kestra** | [kestra-io/kestra](https://github.com/kestra-io/kestra) | ⭐ 26.2k | Modern YAML-first, event-driven |
| **Prefect** | [PrefectHQ/prefect](https://github.com/PrefectHQ/prefect) | ⭐ 21.3k | Python-first, modern Airflow alternative |
| **Temporal** | [temporalio/temporal](https://github.com/temporalio/temporal) | ⭐ 17.5k | Durable execution, saga patterns |
| **Dagster** | [dagster-io/dagster](https://github.com/dagster-io/dagster) | ⭐ 14.7k | Asset-based, data lineage focused |
| **Mage** | [mage-ai/mage-ai](https://github.com/mage-ai/mage-ai) | ⭐ 8.6k | Notebook-style, data engineering |

**Key Insight:** Kestra (26.2k ⭐) and Prefect (21.3k ⭐) are gaining fast. YAML-first and Python-first patterns are both popular.

## Python-First Tools

### Airflow
- **Official TaskFlow API Documentation**: [TaskFlow - Airflow 3.1.5](https://airflow.apache.org/docs/apache-airflow/stable/tutorial/taskflow.html)
- **DAG Concepts**: [DAGs - Airflow 3.1.5](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html)
- **TaskFlow Core Concepts**: [TaskFlow - Airflow 3.1.5](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/taskflow.html)

### Prefect
- **Official Tasks Documentation**: [Tasks - Prefect Docs](https://docs.prefect.io/latest/concepts/tasks/)
- **Official Flows Documentation**: [Flows - Prefect v3](https://docs.prefect.io/v3/concepts/flows)
- **Write and Run Tasks**: [Write and run tasks - Prefect](https://docs.prefect.io/v3/develop/write-tasks)
- **Write and Run Flows**: [Write and run flows - Prefect](https://docs.prefect.io/v3/develop/write-flows)

### Dagster
- **Asset Jobs Guide**: [Asset jobs | Dagster Docs](https://docs.dagster.io/guides/build/jobs/asset-jobs)
- **Jobs Overview**: [Jobs | Dagster Docs](https://docs.dagster.io/guides/build/jobs)
- **Assets API Reference**: [assets | Dagster Docs](https://docs.dagster.io/api/dagster/assets)
- **Defining Assets**: [Defining assets | Dagster Docs](https://docs.dagster.io/concepts/assets/software-defined-assets)

### Temporal
- **Python SDK Developer Guide**: [Python SDK developer guide | Temporal Platform Documentation](https://docs.temporal.io/develop/python)
- **Core Application**: [Core application - Python SDK | Temporal Platform Documentation](https://docs.temporal.io/develop/python/core-application)
- **Official Python SDK**: [temporalio/sdk-python - GitHub](https://github.com/temporalio/sdk-python)
- **Python Samples Repository**: [temporalio/samples-python - GitHub](https://github.com/temporalio/samples-python)
- **Learn Temporal Tutorial**: [Build a Temporal Application from scratch in Python | Learn Temporal](https://learn.temporal.io/getting_started/python/hello_world_in_python/)

### Mage
- **Official Documentation**: [Mage Docs](https://docs.mage.ai/)
- **GitHub Repository**: [mage-ai/mage-ai - GitHub](https://github.com/mage-ai/mage-ai)
- **Pipelines Overview**: [Pipelines Overview](https://docs.mage.ai/design/core-abstractions#pipeline)

## YAML-First Tools

### Google Cloud Workflows
- **Official Overview**: [Workflows overview | Google Cloud Documentation](https://docs.cloud.google.com/workflows/docs/overview)
- **Syntax Reference**: [Syntax overview | Workflows | Google Cloud](https://cloud.google.com/workflows/docs/reference/syntax)
- **Official Samples Repository**: [GoogleCloudPlatform/workflows-samples - GitHub](https://github.com/GoogleCloudPlatform/workflows-samples)
- **Syntax Cheat Sheet**: [workflows-demos/syntax-cheat-sheet/workflow.yaml](https://github.com/GoogleCloudPlatform/workflows-demos/blob/master/syntax-cheat-sheet/workflow.yaml)
- **All Code Samples**: [All Workflows code samples | Google Cloud](https://cloud.google.com/workflows/docs/samples)

### Kestra
- **Official Documentation**: [Welcome to Kestra](https://kestra.io/docs)
- **Tutorial**: [Tutorial | Kestra](https://kestra.io/docs/tutorial)
- **Flow Components**: [Flow | Kestra](https://kestra.io/docs/workflow-components/flow)
- **Official Examples Repository**: [kestra-io/examples - GitHub](https://github.com/kestra-io/examples)
- **GitHub Main Repository**: [kestra-io/kestra - GitHub](https://github.com/kestra-io/kestra)

## Comparison Summary

| Tool | Best Link | Style |
|------|-----------|-------|
| **Airflow** | [TaskFlow Tutorial](https://airflow.apache.org/docs/apache-airflow/stable/tutorial/taskflow.html) | Python @task/@dag |
| **Prefect** | [Write Tasks Guide](https://docs.prefect.io/v3/develop/write-tasks) | Python @task/@flow |
| **Dagster** | [Asset Jobs Guide](https://docs.dagster.io/guides/build/jobs/asset-jobs) | Python @asset/@job |
| **Temporal** | [Python SDK Guide](https://docs.temporal.io/develop/python) | Python @activity/@workflow |
| **Mage** | [Mage Docs](https://docs.mage.ai/) | Python @data_loader/@transformer |
| **GCP Workflows** | [Syntax Cheat Sheet](https://github.com/GoogleCloudPlatform/workflows-demos/blob/master/syntax-cheat-sheet/workflow.yaml) | YAML |
| **Kestra** | [Kestra Tutorial](https://kestra.io/docs/tutorial) | YAML |

## Sources

**Airflow:**
- [Pythonic DAGs with the TaskFlow API — Airflow 3.1.5 Documentation](https://airflow.apache.org/docs/apache-airflow/stable/tutorial/taskflow.html)
- [DAGs — Airflow 3.1.5 Documentation](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/dags.html)
- [TaskFlow — Airflow 3.1.5 Documentation](https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/taskflow.html)

**Prefect:**
- [Tasks - Prefect Docs](https://docs.prefect.io/latest/concepts/tasks/)
- [Flows - Prefect](https://docs.prefect.io/v3/concepts/flows)
- [Write and run tasks - Prefect](https://docs.prefect.io/v3/develop/write-tasks)
- [Write and run flows - Prefect](https://docs.prefect.io/v3/develop/write-flows)

**Dagster:**
- [Asset jobs | Dagster Docs](https://docs.dagster.io/guides/build/jobs/asset-jobs)
- [Jobs | Dagster Docs](https://docs.dagster.io/guides/build/jobs)
- [assets | Dagster Docs](https://docs.dagster.io/api/dagster/assets)
- [Defining assets | Dagster Docs](https://docs.dagster.io/concepts/assets/software-defined-assets)

**Temporal:**
- [Python SDK developer guide | Temporal Platform Documentation](https://docs.temporal.io/develop/python)
- [Core application - Python SDK | Temporal Platform Documentation](https://docs.temporal.io/develop/python/core-application)
- [GitHub - temporalio/sdk-python: Temporal Python SDK](https://github.com/temporalio/sdk-python)
- [GitHub - temporalio/samples-python: Samples for working with the Temporal Python SDK](https://github.com/temporalio/samples-python)
- [Build a Temporal Application from scratch in Python | Learn Temporal](https://learn.temporal.io/getting_started/python/hello_world_in_python/)

**Google Cloud Workflows:**
- [Workflows overview | Google Cloud Documentation](https://docs.cloud.google.com/workflows/docs/overview)
- [Syntax overview | Workflows | Google Cloud](https://cloud.google.com/workflows/docs/reference/syntax)
- [GitHub - GoogleCloudPlatform/workflows-samples: This repository contains samples for Cloud Workflows.](https://github.com/GoogleCloudPlatform/workflows-samples)
- [workflows-demos/syntax-cheat-sheet/workflow.yaml at master · GoogleCloudPlatform/workflows-demos](https://github.com/GoogleCloudPlatform/workflows-demos/blob/master/syntax-cheat-sheet/workflow.yaml)
- [All Workflows code samples | Google Cloud](https://cloud.google.com/workflows/docs/samples)

**Kestra:**
- [Welcome to Kestra](https://kestra.io/docs)
- [Tutorial](https://kestra.io/docs/tutorial)
- [Flow](https://kestra.io/docs/workflow-components/flow)
- [GitHub - kestra-io/examples: Best practices for data workflows, integrations with the Modern Data Stack (MDS), Infrastructure as Code (IaC), Cloud Provider Services](https://github.com/kestra-io/examples)
- [GitHub - kestra-io/kestra: Event Driven Orchestration & Scheduling Platform for Mission Critical Applications](https://github.com/kestra-io/kestra)
