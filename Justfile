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

# Create + trigger an example workflow by folder name (expects examples/workflows/<name>/<name>.yaml).
example name:
    #!/usr/bin/env bash
    set -euo pipefail
    export DATABASE_URL=${DATABASE_URL:-sqlite://./.ork/ork.db?mode=rwc}
    mkdir -p .ork
    cargo run -q -p ork-cli --no-default-features --features sqlite,process --bin ork -- init >/dev/null 2>&1 || true
    file="examples/workflows/{{name}}/{{name}}.yaml"
    if [ ! -f "$file" ]; then
        set -- examples/workflows/{{name}}/*.yaml
        if [ "$1" = "examples/workflows/{{name}}/*.yaml" ]; then
            echo "No workflow yaml found for examples/workflows/{{name}}" >&2
            exit 1
        fi
        if [ -n "${2-}" ]; then
            echo "Multiple workflow yamls found for examples/workflows/{{name}}; specify one explicitly." >&2
            exit 1
        fi
        file="$1"
    fi
    cargo run -p ork-cli --no-default-features --features sqlite,process --bin ork -- execute --file "$file"

example-all:
    #!/usr/bin/env bash
    set -euo pipefail
    for dir in examples/workflows/*; do
        [ -d "$dir" ] || continue
        name="$(basename "$dir")"
        echo "==> $name"
        just example "$name"
    done

# Start only the scheduler (DB-backed).
run-scheduler:
    cargo run -p ork-cli --bin ork -- run

# Start only the web UI/API.
run-web:
    cargo run -p ork-web -- --addr 127.0.0.1:4000

# Show persisted runs/status (DB-backed).
status:
    cargo run -p ork-cli --bin ork -- status

# Run all tests
test:
    @echo "Running all tests..."
    cargo test --workspace

# Run tests with output
test-verbose:
    @echo "Running all tests with output..."
    cargo test --workspace -- --nocapture

# Run tests for SQLite backend
test-sqlite:
    @echo "Running SQLite tests..."
    cargo test --workspace --no-default-features --features sqlite,process

# Run tests for Postgres backend
test-postgres:
    @echo "Running Postgres tests..."
    cargo test --workspace --features postgres

# Run a specific test by name
test-one name:
    @echo "Running test: {{name}}"
    cargo test --workspace {{name}} -- --nocapture --exact

# Run tests and generate coverage report (requires cargo-tarpaulin)
test-coverage:
    @echo "Generating test coverage..."
    cargo tarpaulin --workspace --out Html --output-dir coverage

# Run all linting checks.
lint:
    ./scripts/check-loc.py
    ./scripts/check-docs.py --check

# Run tests and lint checks
check: test lint
    @echo "All checks passed!"
