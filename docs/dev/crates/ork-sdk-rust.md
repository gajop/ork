# ork-sdk-rust

Rust SDK for authoring task functions and deferrable payloads consumed by Ork runtimes.

## Owns

- Task IO helpers for JSON input/output contract
- Task marker traits/macros for runtime compatibility
- Deferrable payload types for long-running external jobs

## Files

| File | Purpose | Updated | File SHA |
|------|---------|---------|----------|
| [Cargo.toml](../../../crates/ork-sdk-rust/Cargo.toml) | Crate manifest and SDK dependency definitions. | 2026-02-06 | 0e3d0a3 |
| [src/deferrables.rs](../../../crates/ork-sdk-rust/src/deferrables.rs) | Deferrable job types and constructors shared by task authors. | 2026-02-07 | d3f393c |
| [src/lib.rs](../../../crates/ork-sdk-rust/src/lib.rs) | Public SDK API for task input/output and library macro exports. | 2026-02-07 | db7424c |
