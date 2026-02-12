# TODO

## Context
This repo just completed a strict workflow-schema overhaul:
- moved to explicit `inputs` bindings with `const`/`ref`
- introduced strict builtins (`str`, `int`, `float`, `bool`, `date`, `datetime_notz`, `datetime_tz`, `datetime_utc`)
- removed implicit upstream injection in execution
- added `ork validate-workflow` CLI command
- migrated examples/tests to new schema

`just test` is green as of this handoff.

## Completed Work

### ✓ 1) Unified schema enforcement between CLI validator and runtime/core
**Completed:** Removed unsupported features from CLI validator to match runtime capabilities.
- Removed `params` support from CLI validator (not implemented in runtime)
- Removed `run_if` support from CLI validator (not implemented in runtime)
- CLI now only validates `tasks.<task>.output` refs, matching runtime behavior
- Both CLI and runtime now enforce identical schema rules

### ✓ 2) Input binding strictness now identical in core and CLI
**Completed:** Core validation now enforces same strictness as CLI validator.
- Core now requires `inputs` bindings when `input_type` has fields
- Enforced exact key parity between `input_type` and `inputs`
- Added validation that `input_type` must be an object type
- Tests updated to include proper `inputs` bindings

### ✓ 3) Removed legacy input fields from internal types
**Completed:** All legacy fields removed from core structs.
- Removed `TaskDefinition.input` from `crates/ork-core/src/workflow.rs`
- Removed `CompiledTask.input` from `crates/ork-core/src/compiled.rs`
- Removed `TaskSpec.input` and `TaskSpec.upstream` from `crates/ork-core/src/types.rs`
- Updated all tests to remove references to legacy fields
- Updated ork-state tests to remove legacy field usage

### ✓ 4) Aligned docs to final implemented behavior
**Completed:** Documentation updated to match actual runtime behavior.
- Removed `params` section from workflow schema docs
- Removed `run_if` section from task schema docs
- Removed `params.<name>` from reference path grammar
- Updated status to reflect implemented runtime behavior
- Docs now accurately describe only supported features

## Validation Results
All tests passing:
- ✓ `cargo test -p ork-core` - 81 tests passed
- ✓ `cargo test -p ork-cli` - 47 tests passed
- ✓ `cargo test -p ork-state` - All tests passed
- ✓ Updated all workflow examples and tests to use new strict schema
- ✓ CLI validator and runtime now enforce identical rules

## Notes
- No legacy-compat migration mode is desired.
- Keep schema explicit and strict; avoid implicit upstream/input behaviors.
