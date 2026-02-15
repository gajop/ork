# Releasing Ork Crates

This guide covers Rust crate release prep and publish flow for Ork.

## Scope

- This is for crates.io publishing (not PyPI).
- It assumes the workspace metadata and publish-prep script in this repo are used.
- For per-release execution tracking, use [Release Checklist](release-checklist.md).

## Package Graph

Publish in dependency order:

1. `ork-core`
2. `ork-sdk-rust`
3. `ork-executors`
4. `ork-state`
5. `ork-web`
6. `ork-worker`
7. `ork-cli`

## Local Preflight

Run these from the repo root:

```bash
just lint
cargo check --workspace
just publish-prep
```

Notes:

- `just publish-prep` runs packaging manifest checks (`cargo package --list`) and skips publish dry-run by default.
- `just publish-prep-full` enables publish dry-runs (`cargo publish --dry-run`) when you need full end-to-end verification.

## Version Bump

Set the release version in workspace metadata:

- Update `version` in `Cargo.toml` under `[workspace.package]`.
- Run `cargo check --workspace`.
- Commit the version change before publishing.

## First Publish (Bootstrap)

For the first crates.io publish of this crate family, some crates depend on others as `dev-dependencies`, so local verify can fail before all crates exist on crates.io.

Use publish with `--no-verify` in dependency order:

```bash
cargo publish -p ork-core --locked --no-verify
cargo publish -p ork-sdk-rust --locked --no-verify
cargo publish -p ork-executors --locked --no-verify
cargo publish -p ork-state --locked --no-verify
cargo publish -p ork-web --locked --no-verify
cargo publish -p ork-worker --locked --no-verify
cargo publish -p ork-cli --locked --no-verify
```

If crates.io index propagation lags, wait briefly and retry the next crate.

## Subsequent Releases

After initial bootstrap, run full dry-run checks:

```bash
just publish-prep-full
```

Then publish in the same order (typically without `--no-verify`):

```bash
cargo publish -p ork-core --locked
cargo publish -p ork-sdk-rust --locked
cargo publish -p ork-executors --locked
cargo publish -p ork-state --locked
cargo publish -p ork-web --locked
cargo publish -p ork-worker --locked
cargo publish -p ork-cli --locked
```

## CI Integration

- Baseline publish-prep runs in CI via `.github/workflows/ci.yml`.
- Full dry-run checks are gated by repository variable `ORK_RUN_PUBLISH_DRY_RUN=1`.
