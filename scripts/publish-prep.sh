#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

# Publish order: foundational libraries first, binaries last.
PACKAGES=(
  ork-core
  ork-executors
  ork-state
  ork-web
  ork-worker
  ork-sdk-rust
  ork-cli
)

allow_dirty_args=()
if [[ "${ORK_ALLOW_DIRTY:-0}" == "1" ]]; then
  allow_dirty_args+=(--allow-dirty)
fi

package_verify_args=()
# Before first publish, cross-crate dev-deps are not resolvable from crates.io.
if [[ "${ORK_PACKAGE_VERIFY:-0}" != "1" ]]; then
  package_verify_args+=(--no-verify)
fi

echo "==> Packaging manifest checks (cargo package --list)"
for pkg in "${PACKAGES[@]}"; do
  echo "  - cargo package --list -p ${pkg}"
  cargo package --list -p "${pkg}" --locked "${allow_dirty_args[@]}" >/dev/null
done

if [[ "${ORK_PACKAGE_TARBALL:-0}" == "1" ]]; then
  echo "==> Tarball creation checks (cargo package)"
  for pkg in "${PACKAGES[@]}"; do
    echo "  - cargo package -p ${pkg}"
    cargo package -p "${pkg}" --locked \
      "${package_verify_args[@]}" "${allow_dirty_args[@]}"
  done
fi

if [[ "${ORK_SKIP_PUBLISH_DRY_RUN:-0}" == "1" ]]; then
  echo "Skipping publish dry-run (ORK_SKIP_PUBLISH_DRY_RUN=1)."
  exit 0
fi

if [[ "${ORK_RUN_PUBLISH_DRY_RUN:-0}" != "1" ]]; then
  echo "Skipping publish dry-run by default."
  echo "Set ORK_RUN_PUBLISH_DRY_RUN=1 to enable cargo publish --dry-run checks."
  exit 0
fi

publish_verify_args=()
# Keep dry-runs useful before interdependent crates are published.
if [[ "${ORK_PUBLISH_VERIFY:-0}" != "1" ]]; then
  publish_verify_args+=(--no-verify)
fi

echo "==> Publish dry-run checks"
for pkg in "${PACKAGES[@]}"; do
  echo "  - cargo publish -p ${pkg} --dry-run"
  cargo publish -p "${pkg}" --dry-run --locked \
    "${publish_verify_args[@]}" "${allow_dirty_args[@]}"
done
