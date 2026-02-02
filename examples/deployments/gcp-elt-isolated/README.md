# GCP Deployment: ELT Isolated Permissions

Three worker service accounts with minimal permissions each. Extract has internet access, transform is isolated, load bridges GCS to BigQuery.

**Use when:** High-security requirements, need to isolate permissions between pipeline stages.

## What Gets Deployed

- Firestore database
- GCS buckets (ork state, raw data)
- Artifact Registry
- VPC network with connectors (external + internal)
- Service accounts:
  - Ork Scheduler (manages runs, dispatches tasks)
  - Extract Worker (internet access, write to raw bucket)
  - Transform Worker (BigQuery only, no internet)
  - Load Worker (read raw bucket, write to BigQuery)
- Ork Scheduler (Cloud Run service)
- Ork API (Cloud Run service)

## Security Model

- **Extract**: External API access, write to GCS raw bucket only
- **Transform**: BigQuery only, no GCS, no internet (VPC with egress blocked)
- **Load**: Read GCS raw bucket, write to BigQuery

## Permissions

| Service Account | Roles |
|-----------------|-------|
| Ork Scheduler | `datastore.user`, `run.developer`, `storage.objectAdmin` (ork bucket) |
| Extract Worker | `storage.objectCreator` (raw bucket only) |
| Transform Worker | `bigquery.dataEditor`, `bigquery.jobUser` |
| Load Worker | `storage.objectViewer` (raw bucket), `bigquery.dataEditor`, `bigquery.jobUser` |

## Usage

1. Update `workers.yaml` and `locations.yaml` with your project details

2. Deploy infrastructure:

```bash
cd terraform
terraform init
terraform apply -var="project=my-project"
```

3. Workflows reference worker configs by name:

```yaml
name: my_etl

tasks:
  extract:
    worker: extract
    executor: python
    file: tasks/extract.py

  transform:
    worker: transform
    executor: bigquery
    config_file: sql/transform.yaml
    depends_on: [extract]

  load:
    worker: load
    executor: python
    file: tasks/load.py
    depends_on: [transform]
```
