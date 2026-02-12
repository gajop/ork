# Workflow Snapshots Implementation Plan

## Status: IN PROGRESS

## Problem
When users deploy a new version of a workflow via `ork run workflows/example.yaml`, the current behavior either:
1. Reuses the old workflow (preserves history but uses stale definition)
2. Deletes and recreates (updates definition but loses all run history)

Both are incorrect. We need to preserve run history while allowing workflow definitions to evolve.

## Solution: Workflow Snapshots

### Core Concept
- **Workflow** = logical entity (name, scheduling, metadata)
- **WorkflowSnapshot** = immutable version of task definitions
- **Run** = always references a specific snapshot
- Workflow has `current_snapshot_id` pointing to latest version

### Database Schema

**New Table: workflow_snapshots**
- `id` (UUID, PK)
- `workflow_id` (UUID, FK to workflows)
- `content_hash` (SHA256 of tasks JSON) - for deduplication
- `tasks_json` (JSONB/TEXT) - the DAG definition
- `created_at`

**Modified Tables:**
- `workflows` - add `current_snapshot_id` (UUID, nullable FK to workflow_snapshots)
- `runs` - add `snapshot_id` (UUID, FK to workflow_snapshots)

### Implementation Progress

✅ **Completed:**
1. Database migrations (SQLite + Postgres)
2. WorkflowSnapshot model added to models.rs
3. Updated Workflow model with current_snapshot_id
4. Updated Run model with snapshot_id
5. WorkflowSnapshotRepository trait defined in database.rs
6. Database trait updated to require WorkflowSnapshotRepository

⏳ **In Progress:**
- Implementing WorkflowSnapshotRepository for backends

🔲 **TODO:**
1. Implement WorkflowSnapshotRepository for:
   - SQLite (`crates/ork-state/src/sqlite/`)
   - Postgres (`crates/ork-state/src/postgres/`)
   - FileDatabase (`crates/ork-state/src/file_database/`)

2. Update workflow creation flow:
   - `create_workflow` should create initial snapshot
   - `create_workflow_from_yaml` should compute hash and create/reuse snapshot
   - Set `workflow.current_snapshot_id`

3. Update run creation:
   - `create_run` should use `workflow.current_snapshot_id` as `run.snapshot_id`

4. Update task loading:
   - Scheduler should load tasks from `run.snapshot_id` not `workflow_id`
   - This ensures historical runs display correctly

5. Update `execute.rs`:
   - When workflow exists: compute hash, create/reuse snapshot, update current_snapshot_id
   - When workflow doesn't exist: create workflow + snapshot

6. Update tests to verify:
   - Snapshots are deduplicated by hash
   - Historical runs show correct task definitions
   - New runs use latest snapshot

### Hash Computation

Use SHA256 of canonical JSON representation of tasks:
```rust
use sha2::{Sha256, Digest};
use serde_json::json;

fn compute_content_hash(tasks: &[WorkflowTask]) -> String {
    let canonical = serde_json::to_string(&tasks).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

### Migration Path

1. Run migrations (adds nullable columns)
2. Existing workflows/runs continue to work (snapshot_id is nullable)
3. New workflows/runs use snapshots
4. Optional: Backfill snapshots for existing workflows

## Files Modified

- `crates/ork-state/migrations_sqlite/008_add_workflow_snapshots.sql`
- `crates/ork-state/migrations/013_add_workflow_snapshots.sql`
- `crates/ork-core/src/models.rs`
- `crates/ork-core/src/database.rs`
- (TODO) `crates/ork-state/src/sqlite/workflows.rs`
- (TODO) `crates/ork-state/src/postgres/workflows.rs`
- (TODO) `crates/ork-state/src/file_database/workflows.rs`
- (TODO) `crates/ork-cli/src/commands/execute.rs`

## Testing Strategy

1. Unit tests for hash computation
2. Integration tests for snapshot deduplication
3. Test that historical runs display correctly
4. Test that workflow updates create new snapshots
5. Test that identical workflows reuse snapshots
