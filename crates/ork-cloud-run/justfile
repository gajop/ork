# Rust Cloud Run Orchestrator - Justfile
# Run `just --list` to see all available commands

# Default recipe - show help
default:
    @just --list

# Build the project
build:
    cargo build

# Build release version
build-release:
    cargo build --release

# Run tests
test:
    cargo test

# Check compilation without building
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Run clippy linter
lint:
    cargo clippy

# Start PostgreSQL with Docker Compose
db-start:
    docker-compose up -d postgres
    @echo "Waiting for PostgreSQL to be ready..."
    @sleep 3

# Stop PostgreSQL
db-stop:
    docker-compose down

# Initialize database (run migrations)
init: db-start
    cargo run -- init

# Run the orchestrator scheduler
run: db-start
    cargo run -- run

# Create an example workflow (process executor)
create-example-workflow:
    cargo run -- create-workflow \
        --name "example-process" \
        --description "Example workflow with process executor" \
        --job-name "example-task.sh" \
        --project "local" \
        --region "local" \
        --task-count 5 \
        --executor process

# Create a Cloud Run workflow
create-cloudrun-workflow name job project="my-project" region="us-central1" tasks="3":
    cargo run -- create-workflow \
        --name {{name}} \
        --job-name {{job}} \
        --project {{project}} \
        --region {{region}} \
        --task-count {{tasks}} \
        --executor cloudrun

# List all workflows
list-workflows:
    cargo run -- list-workflows

# Trigger a workflow by name
trigger workflow:
    cargo run -- trigger {{workflow}}

# Show status of all runs
status:
    cargo run -- status

# Show status of a specific run
status-run run_id:
    cargo run -- status {{run_id}}

# Show status of runs for a workflow
status-workflow workflow:
    cargo run -- status --workflow {{workflow}}

# Show tasks for a run
tasks run_id:
    cargo run -- tasks {{run_id}}

# Full stack up with Docker Compose
up:
    docker-compose up -d

# Full stack down
down:
    docker-compose down

# View orchestrator logs
logs:
    docker-compose logs -f orchestrator

# Run end-to-end test with process executor
test-e2e: init create-example-workflow
    #!/usr/bin/env bash
    set -e
    echo "Creating example test script..."
    just create-test-script

    echo "Starting orchestrator in background..."
    cargo run -- run &
    ORCH_PID=$!

    echo "Waiting for orchestrator to start..."
    sleep 2

    echo "Triggering workflow..."
    RUN_ID=$(cargo run -- trigger example-process | grep "Run ID:" | awk '{print $3}')
    echo "Started run: $RUN_ID"

    echo "Waiting for tasks to complete..."
    sleep 10

    echo "Checking status..."
    cargo run -- status $RUN_ID
    cargo run -- tasks $RUN_ID

    echo "Stopping orchestrator..."
    kill $ORCH_PID || true

    echo "Test complete!"

# Create test script for process executor
create-test-script:
    #!/usr/bin/env bash
    mkdir -p test-scripts
    cat > test-scripts/example-task.sh << 'EOF'
    #!/bin/bash
    echo "Starting task execution..."
    echo "Task index: ${TASK_INDEX}"
    echo "Run ID: ${RUN_ID}"
    echo "Workflow: ${WORKFLOW_NAME}"

    # Simulate some work
    sleep 2

    echo "Task completed successfully!"

    # Output result as JSON
    echo '{"status": "success", "processed_items": 42}'
    EOF
    chmod +x test-scripts/example-task.sh
    echo "Created test-scripts/example-task.sh"

# Clean up test artifacts
clean-test:
    rm -rf test-scripts
    cargo run -- status --workflow example-process || true

# Clean build artifacts
clean:
    cargo clean
    rm -rf target

# Reset database (WARNING: deletes all data)
reset-db: db-stop
    docker-compose down -v
    @echo "Database reset complete"

# Development workflow - rebuild and restart
dev: build
    just run

# Docker: Build the image
docker-build:
    docker build -t rust-orchestrator .

# Docker: Run with compose
docker-up:
    docker-compose up -d
    @echo "Waiting for services to start..."
    @sleep 5
    docker-compose exec orchestrator /app/rust-orchestrator init

# Docker: Execute command in orchestrator container
docker-exec *ARGS:
    docker-compose exec orchestrator /app/rust-orchestrator {{ARGS}}

# Docker: Create example workflow in Docker
docker-create-example:
    docker-compose exec orchestrator /app/rust-orchestrator create-workflow \
        --name "docker-example" \
        --job-name "example-task" \
        --project "my-project" \
        --task-count 3 \
        --executor process

# Docker: Trigger workflow in Docker
docker-trigger workflow:
    docker-compose exec orchestrator /app/rust-orchestrator trigger {{workflow}}

# Docker: View status in Docker
docker-status:
    docker-compose exec orchestrator /app/rust-orchestrator status

# Docker: View logs
docker-logs:
    docker-compose logs -f orchestrator

# Show database tables
db-tables:
    docker-compose exec postgres psql -U postgres -d orchestrator -c "\dt"

# Query workflows table
db-workflows:
    docker-compose exec postgres psql -U postgres -d orchestrator -c "SELECT id, name, cloud_run_job_name, executor_type FROM workflows;"

# Query runs table
db-runs:
    docker-compose exec postgres psql -U postgres -d orchestrator -c "SELECT id, workflow_id, status, created_at FROM runs ORDER BY created_at DESC LIMIT 10;"

# Query tasks table
db-tasks run_id:
    docker-compose exec postgres psql -U postgres -d orchestrator -c "SELECT task_index, status, cloud_run_execution_name, finished_at FROM tasks WHERE run_id = '{{run_id}}' ORDER BY task_index;"

# Complete setup for first time
setup: db-start init create-test-script create-example-workflow
    @echo ""
    @echo "Setup complete! You can now:"
    @echo "  just trigger example-process  - Trigger the example workflow"
    @echo "  just run                       - Start the orchestrator"
    @echo "  just status                    - Check run status"
