#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT_DIR"

mkdir -p .ork
export DATABASE_URL=${DATABASE_URL:-sqlite://./.ork/ork.db?mode=rwc}

cargo run -p ork-cli --no-default-features --features sqlite,process --bin ork -- init

echo "Starting scheduler (SQLite) (Ctrl-C to stop)..."
cargo run -p ork-cli --no-default-features --features sqlite,process --bin ork -- run &
SCHED_PID=$!

echo "Starting web UI at http://127.0.0.1:4000 ..."
cargo run -p ork-web --no-default-features --features sqlite -- --addr 127.0.0.1:4000 &
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
