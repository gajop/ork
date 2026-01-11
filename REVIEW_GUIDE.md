# Documentation Review Guide

## Core Design Principles

1. **Low/no cost at idle** - Architecture designed to minimize consumption when not running
2. **Parallelization included** - Built-in support for parallel execution and fan-out patterns
3. **Orchestration as afterthought** - Add orchestration to existing code without rewriting it
4. **Fully open source** - No enterprise edition, no feature gating

## Review Process

**For each doc, check:**

1. **Naming consistency**
   - Only Ork components get "Ork" prefix: Ork Scheduler, Ork API, Ork Worker
   - User-defined things (workers, service accounts) should NOT have "Ork" prefix
   - Domain-specific concepts (ETL/ELT, Extract/Transform/Load) are NOT Ork concepts

2. **Status header**
   - All docs must have "Status: Pending Review" at the top

3. **Design alignment**
   - Does it emphasize near-zero idle cost?
   - Does it keep orchestration separate from business logic?
   - Does it avoid over-engineering?

4. **User clarity**
   - Can a new user understand this?
   - Are examples concrete and realistic?
   - Does it teach concepts, not just syntax?

5. **Technical accuracy**
   - Are links correct?
   - Are paths/references valid?
   - Is Terraform/code valid?

6. **Avoid common pitfalls**
   - Don't conflate deployment architecture with system architecture
   - Don't call things "simple" - be specific about what they do
   - Don't add "Ork" to everything - only actual Ork components
   - Don't put infra/data team split in top-level docs - it's just one deployment model

## Review Order (User Journey)

1. quickstart.md (needs replacement with simple 2-task example)
2. tutorial_elt_pipeline.md (realistic ELT example - current quickstart content)
3. concepts.md
4. workflows.md
5. tasks.md
6. executors.md
7. parallelization.md
8. dbt.md
9. context.md
10. operations.md
11. configuration.md
12. failure_handling.md
13. type_schemas.md
14. deployment.md
15. deployment/gcp/near-zero-cost.md
16. deployment/local.md
17. dev/architecture.md
18. dev/schema.md
19. dev/spec.md
20. dev/crates.md
21. dev/implementation_plan.md

Each doc reviewed twice - first pass for major issues, second pass for polish.

**Note**: tutorial_elt_pipeline.md is intentionally complex - it shows realistic usage with medallion architecture, DuckDB, and parallel processing. Realistic examples demonstrate real-world patterns that contrived examples cannot.
