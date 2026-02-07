set shell := ["sh", "-c"]

default:
    @just --list

# Build all Rust crates in workspace.
build:
    cargo build --workspace

# Build optimized binaries.
build-release:
    cargo build --workspace --release

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
    cargo check --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    ./scripts/check-loc.py
    ./scripts/check-docs.py --check

# Run lint checks in autofix mode where possible, then enforce non-fixable checks.
lint-fix:
    cargo check --workspace
    cargo clippy --workspace --all-targets --fix --allow-dirty --allow-staged
    ./scripts/check-loc.py
    ./scripts/check-docs.py --check

# List available workflow examples.
examples-list:
    @find examples/workflows -mindepth 1 -maxdepth 1 -type d -printf "%f\n" | sort

# Run all workflow examples and keep a machine-readable status matrix.
examples-check-all timeout_s='240':
    #!/usr/bin/env bash
    set -euo pipefail
    timeout_value="{{timeout_s}}"
    timeout_value="${timeout_value#timeout_s=}"
    mkdir -p .ork/reports
    results=.ork/reports/examples-check.tsv
    : > "$results"
    printf "example\tstatus\texit_code\tduration_s\n" >> "$results"
    failed=0
    for dir in examples/workflows/*; do
        [ -d "$dir" ] || continue
        name="$(basename "$dir")"
        log=".ork/reports/example-${name}.log"
        start="$(date +%s)"
        if timeout "$timeout_value" just example "$name" >"$log" 2>&1; then
            ex_status="PASS"
            ex_code=0
        else
            ex_code=$?
            if [ "$ex_code" -eq 124 ]; then
                ex_status="TIMEOUT"
            else
                ex_status="FAIL"
            fi
            failed=1
        fi
        end="$(date +%s)"
        duration="$((end-start))"
        printf "%s\t%s\t%s\t%s\n" "$name" "$ex_status" "$ex_code" "$duration" >> "$results"
        echo "$name: $ex_status (exit=$ex_code, ${duration}s)"
    done
    echo "Wrote $results"
    if [ "$failed" -ne 0 ]; then
        echo "At least one example failed. See .ork/reports/example-*.log."
        exit 1
    fi

# Run quality checks + example matrix and emit a markdown report.
quality-report report='docs/reports/quality/latest.md' timeout_s='240':
    #!/usr/bin/env bash
    set -euo pipefail
    report_path="{{report}}"
    report_path="${report_path#report=}"
    timeout_value="{{timeout_s}}"
    timeout_value="${timeout_value#timeout_s=}"
    mkdir -p .ork/reports "$(dirname "$report_path")"
    build_log=.ork/reports/build.log
    lint_log=.ork/reports/lint.log
    test_log=.ork/reports/test.log

    if just build >"$build_log" 2>&1; then
        build_status="PASS"
        build_code=0
    else
        build_code=$?
        build_status="FAIL"
    fi
    if just lint >"$lint_log" 2>&1; then
        lint_status="PASS"
        lint_code=0
    else
        lint_code=$?
        lint_status="FAIL"
    fi
    if just test >"$test_log" 2>&1; then
        test_status="PASS"
        test_code=0
    else
        test_code=$?
        test_status="FAIL"
    fi
    if just examples-check-all "$timeout_value" >.ork/reports/examples.log 2>&1; then
        examples_status="PASS"
        examples_code=0
    else
        examples_code=$?
        examples_status="FAIL"
    fi

    examples_tsv=.ork/reports/examples-check.tsv
    pass_count=0
    fail_count=0
    timeout_count=0
    if [ -f "$examples_tsv" ]; then
        pass_count="$(awk -F'\t' 'NR>1 && $2=="PASS" {c++} END {print c+0}' "$examples_tsv")"
        fail_count="$(awk -F'\t' 'NR>1 && $2=="FAIL" {c++} END {print c+0}' "$examples_tsv")"
        timeout_count="$(awk -F'\t' 'NR>1 && $2=="TIMEOUT" {c++} END {print c+0}' "$examples_tsv")"
    fi

    generated_at="$(date -u +"%Y-%m-%d %H:%M:%SZ")"
    {
        echo "# Project Quality Report"
        echo
        echo "Generated at: $generated_at (UTC)"
        echo
        echo "## Command Results"
        echo
        echo "| Check | Status | Exit code | Log |"
        echo "|---|---|---:|---|"
        echo "| \`just build\` | $build_status | $build_code | \`.ork/reports/build.log\` |"
        echo "| \`just lint\` | $lint_status | $lint_code | \`.ork/reports/lint.log\` |"
        echo "| \`just test\` | $test_status | $test_code | \`.ork/reports/test.log\` |"
        echo "| \`just examples-check-all $timeout_value\` | $examples_status | $examples_code | \`.ork/reports/examples.log\` |"
        echo
        echo "## Example Matrix Summary"
        echo
        echo "- PASS: $pass_count"
        echo "- FAIL: $fail_count"
        echo "- TIMEOUT: $timeout_count"
        echo "- Matrix file: \`.ork/reports/examples-check.tsv\`"
        echo
        echo "## Notes"
        echo
        echo "- Per-example logs are in \`.ork/reports/example-*.log\`."
        echo "- This report is generated by \`just quality-report\`."
    } > "$report_path"

    echo "Wrote $report_path"
    failed=0
    [ "$build_code" -eq 0 ] || failed=1
    [ "$lint_code" -eq 0 ] || failed=1
    [ "$test_code" -eq 0 ] || failed=1
    [ "$examples_code" -eq 0 ] || failed=1
    exit "$failed"
