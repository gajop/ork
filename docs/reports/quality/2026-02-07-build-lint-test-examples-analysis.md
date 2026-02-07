# Build, Lint, Test, and Examples Analysis

Date: 2026-02-07

## Scope

Executed via `just`:

- `just build`
- `just lint`
- `just test`
- `just examples-check-all 90`

## Summary

- Build: PASS (`exit 0`)
- Lint: FAIL (`exit 1`)
- Test: FAIL (`exit 101`)
- Examples: FAIL (`exit 1`)
- Working examples: `0 / 10`

## Detailed Findings

### 1) Build

- `just build` succeeded.

### 2) Lint

- `just lint` failed due to line-count policy.
- Failing file: `crates/ork-core/src/scheduler.rs` (617 lines, 117 over limit).

### 3) Tests

- `just test` failed during compilation in `crates/ork-sdk-rust/src/lib.rs:233`.
- Error: `TaskOutput` trait is not dyn-compatible (`E0038`) but used as `&dyn TaskOutput`.

### 4) Examples

Results from `.ork/reports/examples-check.tsv`:

| Example | Status | Exit | Duration (s) | Primary Failure Mode |
|---|---|---:|---:|---|
| `branches` | FAIL | 1 | 1 | Missing Python dependency: `pydantic` |
| `deferrables` | FAIL | 1 | 0 | Missing Python dependency: `ork_sdk` |
| `demo` | FAIL | 1 | 1 | Missing Python dependency: `pydantic` |
| `fast` | TIMEOUT | 124 | 90 | Runtime panic: rustls `CryptoProvider` not selected |
| `parallel` | FAIL | 1 | 0 | Missing Python dependency: `pydantic` |
| `polyglot` | TIMEOUT | 124 | 90 | Runtime panic: rustls `CryptoProvider` not selected |
| `quickstart` | FAIL | 1 | 1 | Missing Python dependency: `pydantic` |
| `retries` | FAIL | 1 | 1 | Missing Python dependency: `pydantic` |
| `simple` | TIMEOUT | 124 | 90 | Runtime panic: rustls `CryptoProvider` not selected |
| `simple_args` | TIMEOUT | 124 | 90 | Runtime panic: rustls `CryptoProvider` not selected |

## Quality Assessment

Current project quality is below release-ready:

- Static policy checks fail.
- Test suite does not compile fully.
- All example workflows fail or time out.

Primary blockers are:

1. Python runtime dependencies for examples are not provisioned (`pydantic`, `ork_sdk`).
2. TLS runtime configuration issue causing panic in examples that successfully create and trigger runs.
3. Trait-object type mismatch in SDK tests.

## Justfile Improvements Added

To make `just` the main project interface, new commands were added in `Justfile`:

- `just build`
- `just build-release`
- `just fmt-check`
- `just lint-rust`
- `just quality`
- `just quality-strict`
- `just examples-list`
- `just examples-check-all [timeout_s]`
- `just quality-report [report] [timeout_s]`
