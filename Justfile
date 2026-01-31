set shell := ["sh", "-c"]

default:
    @just --list

# Start Postgres, run migrations, then start the scheduler + web UI.
up:
    ./scripts/dev-up.sh

# Alias for older docs/scripts.
example-up: up

# Create + trigger an example workflow by folder name (expects examples/<name>/<name>.yaml).
example-run name:
    ./scripts/example-run.sh "{{name}}"

# Start only the scheduler (DB-backed).
run-scheduler:
    cargo run -p ork-cli --bin ork -- run

# Start only the web UI/API.
run-web:
    cargo run -p ork-web -- --addr 127.0.0.1:4000

# Show persisted runs/status (DB-backed).
status:
    cargo run -p ork-cli --bin ork -- status
