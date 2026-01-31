# Documentation Index

This is the complete documentation hierarchy for Ork. Start with the [main README](../README.md) for project overview and quick start.

## 🔧 Developer Documentation

### Core Architecture
- **[dev/architecture.md](dev/architecture.md)** - System design, components, data flow, performance
- **[dev/schema.md](dev/schema.md)** - Database schema, migrations, storage formats
- **[dev/crates.md](dev/crates.md)** - Rust crate structure and module organization
- **[dev/spec.md](dev/spec.md)** - Technical specifications and algorithms
- **[performance.md](performance.md)** - Performance testing infrastructure and benchmarks

## 🚀 Deployment

### Working Examples
- **[deployment/local.md](deployment/local.md)** - Local development setup
- **[deployment/gcp/](deployment/gcp/)** - Google Cloud Platform deployment examples

## 🔮 Future Features (Not Yet Implemented)

### Core Concepts
- **[future/concepts.md](future/concepts.md)** - Workflows, tasks, runs, executors
- **[future/workflows.md](future/workflows.md)** - YAML workflow definitions
- **[future/tasks.md](future/tasks.md)** - Task decorators and type validation
- **[future/executors.md](future/executors.md)** - Executor types and configuration

### Advanced Features
- **[future/parallelization.md](future/parallelization.md)** - Parallel execution patterns
- **[future/configuration.md](future/configuration.md)** - Retries, timeouts, resources
- **[future/failure_handling.md](future/failure_handling.md)** - Error handling and recovery
- **[future/context.md](future/context.md)** - Runtime context and metadata
- **[future/operations.md](future/operations.md)** - CLI operations and scheduling

### Integrations
- **[future/dbt.md](future/dbt.md)** - dbt project integration
- **[future/type_schemas.md](future/type_schemas.md)** - Type definitions and validation
- **[future/deployment.md](future/deployment.md)** - Production deployment guide

## 📚 Guides & Tutorials (Work in Progress)

- **[guide/quickstart.md](guide/quickstart.md)** - Getting started guide
- **[guide/tutorial_elt_pipeline.md](guide/tutorial_elt_pipeline.md)** - ELT pipeline tutorial

## 🔍 Comparisons

- **[comparisons/](comparisons/)** - Feature comparisons with other workflow orchestrators
  - Airflow, Prefect, Dagster, Metaflow, Kedro, Luigi, and more

---

## Navigation Tips

**If you're new to Ork:**
1. Start with the [main README](../README.md)
2. Check [deployment/local.md](deployment/local.md) for local setup
3. Read [dev/architecture.md](dev/architecture.md) to understand the system

**If you're contributing:**
1. Review [dev/architecture.md](dev/architecture.md)
2. Check [dev/crates.md](dev/crates.md) for code organization
3. Run [performance.md](performance.md) tests to validate changes

**If you're evaluating Ork:**
1. Read [comparisons/](comparisons/) for feature comparisons
2. Check [dev/architecture.md](dev/architecture.md) for performance characteristics
3. Review [future/](future/) to see planned features
