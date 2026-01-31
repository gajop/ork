#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

export DATABASE_URL=${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/orchestrator}

docker compose -f crates/ork-cli/docker-compose.yml up -d
cargo run -p ork-cli --bin ork -- init

echo "Starting scheduler (Ctrl-C to stop)..."
cargo run -p ork-cli --bin ork -- run &
SCHED_PID=$!

echo "Starting web UI at http://127.0.0.1:4000 ..."
cargo run -p ork-web -- --addr 127.0.0.1:4000 &
WEB_PID=$!

cleanup() {
  kill "$SCHED_PID" "$WEB_PID" 2>/dev/null || true
  wait "$SCHED_PID" "$WEB_PID" 2>/dev/null || true
}

trap cleanup INT TERM

while kill -0 "$SCHED_PID" 2>/dev/null && kill -0 "$WEB_PID" 2>/dev/null; do
  sleep 1
done

cleanup
