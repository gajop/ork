# Ork Release Checklist

Use this checklist for each crates.io release.

## Release Metadata

- Release version: `vX.Y.Z`
- Date (UTC): `YYYY-MM-DD`
- Release owner:
- Notes link:

## 1) Preflight

- [ ] Working tree is clean (or intentional release-only changes are staged)
- [ ] `just lint` passes
- [ ] `cargo check --workspace` passes
- [ ] `just publish-prep` passes
- [ ] `just publish-prep-full` passes (for non-bootstrap releases)

## 2) Versioning

- [ ] `[workspace.package].version` updated in `Cargo.toml`
- [ ] Any release notes/changelog updated
- [ ] Version commit created
- [ ] Release tag created (`vX.Y.Z`)

## 3) Publish Order

Publish in this order:

1. `ork-core`
2. `ork-sdk-rust`
3. `ork-executors`
4. `ork-state`
5. `ork-web`
6. `ork-worker`
7. `ork-cli`

## 4) Publish Commands

Bootstrap publish (first release of crate family):

```bash
cargo publish -p ork-core --locked --no-verify
cargo publish -p ork-sdk-rust --locked --no-verify
cargo publish -p ork-executors --locked --no-verify
cargo publish -p ork-state --locked --no-verify
cargo publish -p ork-web --locked --no-verify
cargo publish -p ork-worker --locked --no-verify
cargo publish -p ork-cli --locked --no-verify
```

Subsequent publishes:

```bash
cargo publish -p ork-core --locked
cargo publish -p ork-sdk-rust --locked
cargo publish -p ork-executors --locked
cargo publish -p ork-state --locked
cargo publish -p ork-web --locked
cargo publish -p ork-worker --locked
cargo publish -p ork-cli --locked
```

Checklist:

- [ ] `ork-core` published
- [ ] `ork-sdk-rust` published
- [ ] `ork-executors` published
- [ ] `ork-state` published
- [ ] `ork-web` published
- [ ] `ork-worker` published
- [ ] `ork-cli` published

## 5) Post-Publish Verification

- [ ] `cargo install ork-cli --bin ork --locked` succeeds
- [ ] `ork --help` runs
- [ ] `ork serve --help` runs
- [ ] `ork-worker --help` runs
- [ ] README install commands validated

## 6) Wrap-Up

- [ ] Push release tag
- [ ] Create GitHub release notes
- [ ] Announce release (if applicable)
- [ ] Record any follow-up issues
