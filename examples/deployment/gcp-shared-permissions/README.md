# GCP Deployment: Shared Permissions

Single worker service account with broad permissions across GCS and BigQuery. All tasks run with the same permissions.

**Use when:** Small team, low-security requirements, simple permission model.

## What Gets Deployed

- Firestore database
- GCS bucket
- Artifact Registry
- Service accounts:
  - Ork Scheduler (manages runs, dispatches tasks)
  - Ork Worker (runs tasks with shared permissions)
- Ork Scheduler (Cloud Run service)
- Ork API (Cloud Run service)

## Permissions

| Service Account | Roles |
|-----------------|-------|
| Ork Scheduler | `datastore.user`, `run.developer`, `storage.objectAdmin` (ork bucket) |
| Ork Worker | `storage.objectAdmin`, `bigquery.dataEditor`, `bigquery.jobUser` |

## Usage

1. Update `workers.yaml` and `locations.yaml` with your project details

2. Deploy infrastructure:

```bash
cd terraform
terraform init
terraform apply -var="project=my-project"
```

3. Push workflow images to Artifact Registry:

```bash
docker build -t us-central1-docker.pkg.dev/my-project/workflows/etl:latest .
docker push us-central1-docker.pkg.dev/my-project/workflows/etl:latest
```

4. Ork automatically discovers and runs workflows
