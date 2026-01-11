# Complete Survey of Workflow Orchestration Frameworks (2026)

Comprehensive list of data engineering, MLOps, and workflow orchestration frameworks with GitHub stars and categories.

## Summary Statistics

- **Total frameworks surveyed:** 24
- **Categories:** General Orchestration (8), MLOps-Specific (6), Data Engineering (4), CI/CD (3), Low-Code/Visual (3)
- **Star range:** 4.1k - 168k stars
- **Most popular:** n8n (168k), Airflow (43.8k), Kestra (26.2k)

## Complete Framework List

### General Workflow Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **n8n** | [n8n-io/n8n](https://github.com/n8n-io/n8n) | ⭐ 168k | TypeScript | 2019 | Fair-code workflow automation with visual editor and 400+ integrations |
| **Airflow** | [apache/airflow](https://github.com/apache/airflow) | ⭐ 43.8k | Python | 2015 | Apache Foundation, most mature, Python-based DAG orchestration |
| **Kestra** | [kestra-io/kestra](https://github.com/kestra-io/kestra) | ⭐ 26.2k | Java | 2020 | Event-driven YAML-first orchestration with modern UI |
| **Prefect** | [PrefectHQ/prefect](https://github.com/PrefectHQ/prefect) | ⭐ 21.3k | Python | 2018 | Python-first, modern Airflow alternative with optional server |
| **Luigi** | [spotify/luigi](https://github.com/spotify/luigi) | ⭐ 18.4k | Python | 2012 | Spotify's batch job framework, handles dependencies and visualization |
| **Temporal** | [temporalio/temporal](https://github.com/temporalio/temporal) | ⭐ 17.5k | Go | 2019 | Durable execution engine for saga patterns and long-running workflows |
| **Argo Workflows** | [argoproj/argo-workflows](https://github.com/argoproj/argo-workflows) | ⭐ 16.3k | Go | 2017 | Kubernetes-native workflow engine, CNCF graduated project |
| **Dagster** | [dagster-io/dagster](https://github.com/dagster-io/dagster) | ⭐ 14.7k | Python | 2018 | Asset-based orchestration focused on data lineage and observability |

### MLOps-Specific Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **Kedro** | [kedro-org/kedro](https://github.com/kedro-org/kedro) | ⭐ 10.7k | Python | 2019 | Production-ready data science pipelines, LF AI & Data Foundation |
| **Metaflow** | [Netflix/metaflow](https://github.com/Netflix/metaflow) | ⭐ 8.8k | Python | 2019 | Netflix's ML/AI workflow framework, now maintained by Outerbounds |
| **Mage** | [mage-ai/mage-ai](https://github.com/mage-ai/mage-ai) | ⭐ 8.6k | Python | 2021 | Notebook-style data pipelines with modern UI, supports Python/SQL/R |
| **Flyte** | [flyteorg/flyte](https://github.com/flyteorg/flyte) | ⭐ 6.7k | Go | 2020 | Kubernetes-native ML orchestration, built for scalability |
| **ZenML** | [zenml-io/zenml](https://github.com/zenml-io/zenml) | ⭐ ~5k* | Python | 2021 | MLOps framework from pipelines to agents, modular and extensible |
| **Kubeflow Pipelines** | [kubeflow/pipelines](https://github.com/kubeflow/pipelines) | ⭐ 4.0k | Python | 2018 | Google-originated ML pipelines for Kubernetes |

### Data Engineering Focused

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **Apache NiFi** | [apache/nifi](https://github.com/apache/nifi) | ⭐ 5.9k | Java | 2014 | Dataflow automation with visual drag-and-drop interface |
| **Orchest** | [orchest/orchest](https://github.com/orchest/orchest) | ⭐ 4.1k | Python | 2020 | Visual pipeline builder for data science workflows |
| **dbt** | [dbt-labs/dbt-core](https://github.com/dbt-labs/dbt-core) | ⭐ ~35k* | Python | 2016 | SQL transformation framework, not orchestration but often used with orchestrators |
| **SQLMesh** | [TobikoData/sqlmesh](https://github.com/TobikoData/sqlmesh) | ⭐ ~2k* | Python | 2022 | dbt alternative with stronger testing, now owned by Fivetran |

### CI/CD and Code-First Orchestration

| Framework | GitHub | Stars | Language | Year | Description |
|-----------|--------|-------|----------|------|-------------|
| **GitHub Actions** | N/A (proprietary) | N/A | YAML | 2019 | GitHub's integrated CI/CD, 71M jobs/day, adding parallel steps in 2026 |
| **GitLab CI** | [gitlab-org/gitlab](https://github.com/gitlab-org/gitlab) | ⭐ ~24k* | Ruby | 2011 | GitLab's integrated CI/CD with advanced DAG pipelines |
| **Jenkins** | [jenkinsci/jenkins](https://github.com/jenkinsci/jenkins) | ⭐ ~23k* | Java | 2011 | Classic CI/CD with massive plugin ecosystem, still widely used |

### Cloud-Managed Services

| Framework | Provider | Cost Model | Year | Description |
|-----------|----------|------------|------|-------------|
| **GCP Workflows** | Google Cloud | Pay-per-execution | 2020 | Serverless YAML-based orchestration for GCP services |
| **AWS Step Functions** | AWS | Pay-per-state-transition | 2016 | Serverless workflow coordination for AWS services |
| **Azure Data Factory** | Microsoft Azure | Pay-per-activity | 2015 | Cloud ETL and data integration service |
| **Databricks Workflows** | Databricks | Included in platform | 2021 | Native orchestration for Databricks jobs and notebooks |

### Low-Code/Visual Platforms

| Framework | GitHub | Stars | Type | Description |
|-----------|--------|-------|------|-------------|
| **n8n** | [n8n-io/n8n](https://github.com/n8n-io/n8n) | ⭐ 168k | Open-source | Fair-code automation with visual workflow builder |
| **Zapier** | N/A (proprietary) | N/A | SaaS | Most popular no-code automation, 6000+ integrations |
| **Make (Integromat)** | N/A (proprietary) | N/A | SaaS | Visual automation platform, strong in Europe |

*Note: Approximate stars, exact count not available in search results

## Framework Categories by Use Case

### Best for Data Engineering Pipelines
1. **Airflow** - Industry standard, massive ecosystem
2. **Prefect** - Modern alternative, easier to use
3. **Dagster** - Best for data lineage and observability
4. **Kestra** - YAML-first, event-driven
5. **dbt** - SQL transformations (needs orchestrator)

### Best for MLOps
1. **Kubeflow Pipelines** - Kubernetes-native, Google-backed
2. **Metaflow** - Battle-tested at Netflix, simple API
3. **Flyte** - Highly scalable, reproducible
4. **ZenML** - Modular, integrates with many tools
5. **Kedro** - Software engineering best practices

### Best for Kubernetes
1. **Argo Workflows** - CNCF graduated, Kubernetes-native
2. **Kubeflow Pipelines** - ML on Kubernetes
3. **Flyte** - Kubernetes-native with strong typing
4. **Airflow** - Can run on Kubernetes (KubernetesExecutor)

### Best for Serverless/Low Infrastructure
1. **GCP Workflows** - Fully managed, pay-per-execution
2. **AWS Step Functions** - AWS-native serverless
3. **Prefect Cloud** - Managed service with hybrid execution
4. **Databricks Workflows** - Integrated with Databricks platform

### Best for Visual/Low-Code
1. **n8n** - 168k stars, self-hostable, 400+ integrations
2. **Apache NiFi** - Enterprise-grade visual dataflow
3. **Kestra** - Modern UI with YAML + visual editor
4. **Zapier/Make** - SaaS, easiest for non-technical users

### Best for CI/CD
1. **GitHub Actions** - 71M jobs/day, native GitHub integration
2. **GitLab CI** - Advanced DAGs, on-prem support
3. **Jenkins** - Maximum flexibility, huge plugin ecosystem
4. **Argo Workflows** - GitOps-friendly, Kubernetes-native

## Key Trends (2026)

### 1. Convergence of Data and ML Orchestration
- MLOps tools adding general orchestration (ZenML, Flyte)
- Data tools adding ML features (Dagster, Prefect)
- Boundaries blurring between categories

### 2. Kubernetes-Native Growth
- Argo Workflows: 16.3k stars (CNCF graduated)
- Flyte, Kubeflow gaining traction
- Airflow adding better K8s support

### 3. Python-First vs YAML-First Divide
**Python-First:** Prefect (21.3k), Dagster (14.7k), Metaflow (8.8k)
**YAML-First:** Kestra (26.2k), GCP Workflows, Argo (16.3k)

Both approaches viable, depends on team preference.

### 4. Serverless and Managed Services
- Cloud providers investing heavily (GCP Workflows, AWS Step Functions, Databricks)
- Reduces operational burden
- Vendor lock-in concerns drive open-source adoption

### 5. Visual/Low-Code Explosion
- n8n: 168k stars (fastest growing)
- Enterprise adoption of visual tools increasing
- Balance between ease-of-use and flexibility

### 6. CI/CD Pricing Pressure
- GitHub Actions introducing self-hosted runner fees (March 2026)
- Driving evaluation of alternatives (GitLab, Argo, etc.)
- Open-source tools gaining traction

## Market Consolidation

### Recent Acquisitions/Mergers
- **Fivetran + dbt Labs** (2025) - Creates dominant ELT + transformation stack
- **Fivetran acquires SQLMesh** (2025) - Brings SQLMesh under same umbrella as dbt
- **Databricks + MosaicML** (2023) - Strengthens ML capabilities

### Impact on Tool Selection
- Vendor lock-in concerns increasing
- Multi-vendor strategies gaining favor
- Open-source tools as hedge against consolidation

## Where Ork Fits

### Current Position
- **Stars:** None (not public yet)
- **Category:** Serverless general orchestration
- **Differentiator:** No server required, truly serverless execution

### Competitive Analysis
**Most similar to:**
1. **Prefect** - Python-first, optional server
2. **GCP Workflows** - Serverless, but cloud-locked
3. **Kestra** - YAML-first, but requires server

**Key advantages Ork could have:**
- ✅ No server required (vs Airflow, Dagster, Kestra, Temporal)
- ✅ No vendor lock-in (vs GCP, AWS, Azure, Databricks)
- ✅ Low/no cost at idle (vs managed services)
- ✅ Local development (vs cloud-only services)

**Current disadvantages:**
- ❌ 70 lines/4 files (vs Prefect 35 lines/1 file)
- ❌ No community/ecosystem yet
- ❌ No track record in production

### Opportunity
The serverless + multi-cloud + open-source niche is underserved. Closest competitor is Prefect, but it still requires optional server for scheduling.

**Ork could be:** "The Prefect of truly serverless orchestration"

## Sources

**Data Orchestration:**
- [Compare Top 15 Data Orchestration Tools in 2026](https://research.aimultiple.com/data-orchestration-tools/)
- [12 Best Open-Source Data Orchestration Tools in 2026 | Airbyte](https://airbyte.com/top-etl-tools-for-sources/data-orchestration-tools)
- [Top Data Orchestration Tools in 2026](https://www.alation.com/blog/data-orchestration-tools/)

**MLOps:**
- [25 Top MLOps Tools You Need to Know in 2026 | DataCamp](https://www.datacamp.com/blog/top-mlops-tools)
- [27 MLOps Tools for 2026: Key Features & Benefits](https://lakefs.io/blog/mlops-tools/)
- [Best Machine Learning Workflow and Pipeline Orchestration Tools](https://neptune.ai/blog/best-workflow-and-pipeline-orchestration-tools)

**GitHub Repositories:**
- [GitHub - apache/airflow](https://github.com/apache/airflow)
- [GitHub - kestra-io/kestra](https://github.com/kestra-io/kestra)
- [GitHub - PrefectHQ/prefect](https://github.com/PrefectHQ/prefect)
- [GitHub - temporalio/temporal](https://github.com/temporalio/temporal)
- [GitHub - dagster-io/dagster](https://github.com/dagster-io/dagster)
- [GitHub - mage-ai/mage-ai](https://github.com/mage-ai/mage-ai)
- [GitHub - argoproj/argo-workflows](https://github.com/argoproj/argo-workflows)
- [GitHub - flyteorg/flyte](https://github.com/flyteorg/flyte)
- [GitHub - Netflix/metaflow](https://github.com/Netflix/metaflow)
- [GitHub - kedro-org/kedro](https://github.com/kedro-org/kedro)
- [GitHub - spotify/luigi](https://github.com/spotify/luigi)
- [GitHub - kubeflow/pipelines](https://github.com/kubeflow/pipelines)
- [GitHub - apache/nifi](https://github.com/apache/nifi)
- [GitHub - n8n-io/n8n](https://github.com/n8n-io/n8n)
- [GitHub - zenml-io/zenml](https://github.com/zenml-io/zenml)
- [GitHub - orchest/orchest](https://github.com/orchest/orchest)

**CI/CD:**
- [Is Github Actions the ultimate workflow orchestration tool? | Orchestra](https://www.getorchestra.io/blog/is-github-actions-the-ultimate-workflow-orchestration-tool)
- [CI/CD Deep Dive: Why Jenkins, GitLab, and CircleCI Still Rule in 2026](https://dev.to/dataformathub/cicd-deep-dive-why-jenkins-gitlab-and-circleci-still-rule-in-2026-268c)
- [GitHub Actions Pricing: 39% Runner Cuts, New Fees from Jan 2026](https://www.webpronews.com/github-actions-pricing-39-runner-cuts-new-fees-from-jan-2026/)

**dbt Alternatives:**
- [Top 10 dbt Alternatives for Data Transformation in 2026](https://hevodata.com/learn/top-dbt-alternatives-data-transformation/)
- [dbt Alternatives: 10 Platforms Compared | Datacoves](https://datacoves.com/post/dbt-alternatives)
