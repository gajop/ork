#!/usr/bin/env bash
set -euo pipefail

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo "This script is meant to be sourced, not executed directly." >&2
  echo "Usage: source scripts/prepare-example-env.sh <example_dir> <project_root>" >&2
  exit 1
fi

example_dir="${1:?example_dir is required}"
project_root="${2:?project_root is required}"

pyproject_dir=""
if [[ -f "$example_dir/pyproject.toml" ]]; then
  pyproject_dir="$example_dir"
else
  shopt -s nullglob
  pyprojects=("$example_dir"/*/pyproject.toml)
  shopt -u nullglob
  if [[ "${#pyprojects[@]}" -eq 1 ]]; then
    pyproject_dir="$(dirname "${pyprojects[0]}")"
  fi
fi

if [[ -n "$pyproject_dir" ]]; then
  if ! command -v uv >/dev/null 2>&1; then
    echo "uv is required for Python example dependencies ($pyproject_dir)." >&2
    return 1
  fi

  PROJECT_ROOT="$project_root" uv sync --project "$pyproject_dir"
  export VIRTUAL_ENV="$pyproject_dir/.venv"
  export PATH="$VIRTUAL_ENV/bin:$PATH"
fi

export PYTHONPATH="$example_dir${PYTHONPATH:+:$PYTHONPATH}"
