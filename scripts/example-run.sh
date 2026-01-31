#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: $0 <example-name>"
  exit 1
fi

name="$1"
file="examples/${name}/${name}.yaml"

if [ ! -f "$file" ]; then
  echo "Missing example file: $file"
  exit 1
fi

workflow_name="$(sed -n 's/^name:[[:space:]]*//p' "$file" | head -n 1)"
if [ -z "$workflow_name" ]; then
  echo "Could not read workflow name from $file"
  exit 1
fi

cargo run -p ork-cli --bin ork -- delete-workflow "$workflow_name" >/dev/null 2>&1 || true
cargo run -p ork-cli --bin ork -- create-workflow-yaml --file "$file" --project local --region local

run_id="$(cargo run -p ork-cli --bin ork -- trigger "$workflow_name" | awk '/Run ID:/ {print $3}')"
if [ -z "$run_id" ]; then
  echo "Failed to read run id for $workflow_name"
  exit 1
fi

while true; do
  status="$(cargo run -p ork-cli --bin ork -- status "$run_id" | awk 'NR==4 {print $3}')"
  if [ -z "$status" ]; then
    echo "Failed to read status for $run_id"
    exit 1
  fi
  echo "Run $run_id status: $status"
  case "$status" in
    success|failed|cancelled) break ;;
  esac
  sleep 2
done
