set shell := ["sh", "-c"]

default:
    @just --list

# Run the demo workflow locally (uses file-backed state and runs dirs)
run-demo:
    cargo run -p ork-cli -- run examples/demo/demo.yaml --run-dir .ork/runs --state-dir .ork/state

run-parallel:
    cargo run -p ork-cli -- run examples/parallel/parallel.yaml --run-dir .ork/runs --state-dir .ork/state

run-branches:
    cargo run -p ork-cli -- run examples/branches/branches.yaml --run-dir .ork/runs --state-dir .ork/state

run-retries:
    cargo run -p ork-cli -- run examples/retries/retries.yaml --run-dir .ork/runs --state-dir .ork/state

run-quickstart:
    cargo run -p ork-cli -- run examples/quickstart/elt_pipeline.yaml --run-dir .ork/runs --state-dir .ork/state

# Serve the minimal web UI + JSON API
serve-ui:
    cargo run -p ork-web -- --run-dir .ork/runs --state-dir .ork/state --addr 127.0.0.1:4000 --parallel 4

# Show persisted runs/status
status:
    cargo run -p ork-cli -- status --run-dir .ork/runs --state-dir .ork/state --show-outputs
