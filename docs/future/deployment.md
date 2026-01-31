Status: Pending Review

# Deployment

This section covers deploying Ork to various environments.

## Available Guides

### GCP

- [Near-Zero Cost](deployment/gcp/near-zero-cost.md) - Serverless deployment using Cloud Run, Firestore, and GCS

### Local Development

- [Local Setup](deployment/local.md) - Running Ork locally with Docker Compose

## Choosing a Deployment

**For development**: Use the [local setup](deployment/local.md) with Docker Compose and Firestore emulator.

**For GCP**: Start with [near-zero cost](deployment/gcp/near-zero-cost.md) which scales to zero when idle.

## Coming Soon

- GCP: Always-on work pool architecture
- AWS: Serverless deployment with DynamoDB and Fargate
- Azure: Serverless deployment with CosmosDB and Container Apps
- Self-hosted: Postgres and Docker/K8s
