# TODO

## Context
This repo just completed a strict workflow-schema overhaul:
- moved to explicit `inputs` bindings with `const`/`ref`
- introduced strict builtins (`str`, `int`, `float`, `bool`, `date`, `datetime_notz`, `datetime_tz`, `datetime_utc`)
- removed implicit upstream injection in execution
- added `ork validate-workflow` CLI command
- migrated examples/tests to new schema

`just test` is green as of this handoff.

## Remaining Work For Next Agent

### 1) Unify schema enforcement between CLI validator and runtime/core
Current mismatch:
- CLI validator (`crates/ork-cli/src/commands/validate_workflow.rs`) supports features not fully represented in core runtime model (`params` refs and `run_if`).
- Runtime/core validation (`crates/ork-core/src/workflow.rs`, `crates/ork-core/src/task_execution.rs`) only supports `tasks.<task>.output...` refs and has no execution semantics for `run_if`.

Action:
- Decide canonical schema owner (recommended: core model + validator aligned to it).
- Either implement missing runtime support (`params`, `run_if`) or remove/reject these from CLI validator + docs until implemented.

### 2) Make input binding strictness identical in core and CLI
Current mismatch:
- CLI validator enforces full `input_type` <-> `inputs` key parity.
- Core `Workflow::validate` currently validates `inputs` only when present and can allow absent bindings.

Action:
- Enforce in core that tasks with non-empty `input_type` require `inputs`, with exact key parity and binding shape checks.
- Add regression tests in `crates/ork-core/src/workflow.rs` tests.

### 3) Finish removing legacy input fields from internal types
Legacy fields still exist in core structs (even though legacy usage is rejected in validation):
- `TaskDefinition.input` in `crates/ork-core/src/workflow.rs`
- `CompiledTask.input` in `crates/ork-core/src/compiled.rs`
- `TaskSpec.input` and `TaskSpec.upstream` in `crates/ork-core/src/types.rs`

Action:
- Remove unused legacy fields and associated test scaffolding.
- Ensure serialization/DB contracts remain stable where needed.

### 4) Align docs to final implemented behavior
`docs/design/workflow-schema.md` currently describes broader validation than runtime in a few areas.

Action:
- Update wording to exactly match implemented runtime behavior after step (1).
- Keep examples minimal and executable.

## Suggested Validation Plan
1. `just test`
2. Add targeted tests for any schema/rules changed:
   - `cargo test -p ork-core workflow::tests::`
   - `cargo test -p ork-cli validate_workflow::tests::`
3. Run `ork validate-workflow` against all workflow examples and ensure behavior matches runtime compile/execute expectations.

## Notes
- No legacy-compat migration mode is desired.
- Keep schema explicit and strict; avoid implicit upstream/input behaviors.
