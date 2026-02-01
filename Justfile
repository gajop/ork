set shell := ["sh", "-c"]

default:
    @just --list

# Start Postgres, run migrations, then start the scheduler + web UI.
up:
    ./scripts/dev-up.sh

# Alias for older docs/scripts.
example-up: up

# Start SQLite-backed scheduler + web UI (no Docker).
up-sqlite:
    ./scripts/dev-up-sqlite.sh

# Create + trigger an example workflow by folder name (expects examples/<name>/<name>.yaml).
example-run name:
    cargo run -p ork-cli --bin ork -- run-workflow --file "examples/{{name}}/{{name}}.yaml"

example-run-sqlite name:
    cargo run -p ork-cli --no-default-features --features sqlite,process --bin ork -- run-workflow --file "examples/{{name}}/{{name}}.yaml"

# Start only the scheduler (DB-backed).
run-scheduler:
    cargo run -p ork-cli --bin ork -- run

# Start only the web UI/API.
run-web:
    cargo run -p ork-web -- --addr 127.0.0.1:4000

# Show persisted runs/status (DB-backed).
status:
    cargo run -p ork-cli --bin ork -- status

# Run all linting checks.
lint:
    ./scripts/check-loc.py
    ./scripts/check-docs.py --check
