# Project Quality Report

Generated at: 2026-02-07 12:03:54Z (UTC)

## Scope

This update focuses on reaching at least 50% coverage in all crates and validating the quality gates via `just`.

## Command Results

| Check | Status | Exit code | Notes |
|---|---|---:|---|
| `just lint` | PASS | 0 | Includes `cargo check`, `cargo clippy -D warnings`, LOC check, docs check. |
| `just test` | PASS | 0 | Unit + integration + doctests (ignored docs remain ignored). |
| `cargo tarpaulin --workspace --out Stdout > .ork/reports/coverage.log` | PASS | 0 | Coverage source for the tables below. |

## Coverage Summary

Overall workspace coverage: **60.12%** (`2631/4376` lines).

### By Crate (tarpaulin totals)

| Crate | Covered | Total | Coverage |
|---|---:|---:|---:|
| `ork-cli` | 290 | 557 | 52.06% |
| `ork-core` | 923 | 1454 | 63.48% |
| `ork-executors` | 328 | 466 | 70.39% |
| `ork-sdk-rust` | 30 | 53 | 56.60% |
| `ork-state` | 563 | 1085 | 51.89% |
| `ork-web` | 443 | 678 | 65.34% |
| `ork-worker` | 54 | 83 | 65.06% |

Result: **all crates are >= 50%**.

### By Crate (`src` only)

| Crate | Covered | Total | Coverage |
|---|---:|---:|---:|
| `ork-cli` | 290 | 557 | 52.06% |
| `ork-core` | 550 | 1052 | 52.28% |
| `ork-executors` | 217 | 351 | 61.82% |
| `ork-sdk-rust` | 30 | 53 | 56.60% |
| `ork-state` | 563 | 1085 | 51.89% |
| `ork-web` | 243 | 448 | 54.24% |
| `ork-worker` | 54 | 83 | 65.06% |

Result: `src` coverage is also **>= 50% for every crate**.

## Low-Coverage Hotspots

These are still weak and good next targets:

- `crates/ork-cli/src/bin/perf-test.rs` (`0/182`)
- `crates/ork-core/src/job_tracker.rs` (`22/160`)
- `crates/ork-core/src/triggerer.rs` (`3/82`)
- `crates/ork-core/src/worker_client.rs` (`3/30`)
- `crates/ork-web/src/handlers.rs` (`42/109`)
- `crates/ork-worker/src/main.rs` (`2/20`)
- Postgres-only `ork-state` modules remain largely untested (multiple `0/N` files)

## Changes Made In This Pass

- Added new unit tests in `crates/ork-core/src/workflow.rs`, `crates/ork-core/src/types.rs`, and `crates/ork-core/src/models.rs` to improve core source coverage.
- Fixed clippy issues in `crates/ork-worker/src/compile.rs` and `crates/ork-cli/src/bin/perf-test.rs`.
- Updated crate docs index in `docs/dev/crates/ork-core.md` so `just lint` docs checks pass.
