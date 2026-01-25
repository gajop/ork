# Performance Testing and Profiling

## Goals

Measure the performance characteristics of the orchestrator:

1. **Throughput**: How many tasks can we process per second over a time interval (10s, 60s)?
2. **Resource Usage**: Memory and CPU consumption of:
   - Scheduler process
   - Worker processes (combined)
3. **Latency**: Time from task creation → dispatch → completion

## Current State

### What We Have
- `just test-load` - triggers 100 workflows but doesn't measure anything
- Process executor with actual subprocess spawning
- No metrics collection
- No performance instrumentation

### What We Need

1. **Metrics Collection**
   - Task throughput counter (tasks/sec)
   - Task latency histogram (created_at → finished_at)
   - System resource monitoring (CPU, memory)

2. **Performance Test Script**
   - Configurable load (task count, concurrency)
   - Real-time metrics output
   - Resource monitoring during test
   - Summary report at end

3. **Test Workflow**
   - Lightweight task that completes quickly
   - Configurable sleep duration to simulate work
   - Should already exist in `test-scripts/example-task.sh`

## Implementation

### Test Binary: `src/bin/perf-test.rs`

The performance test binary (written in Rust):
- Builds a release binary for optimal performance
- Creates a lightweight test workflow with configurable sleep duration
- Starts the scheduler in background
- Triggers N workflows concurrently using Tokio async tasks
- Monitors task completion in real-time
- Tracks scheduler RSS memory and CPU usage via `sysinfo` crate
- Counts active worker processes
- Queries database directly via `sqlx` for latency statistics
- Reports comprehensive metrics

### Metrics Collected

1. **Throughput**
   - Workflows triggered per second
   - Tasks completed per second
   - Total test duration

2. **Latency** (from database timestamps)
   - Average task latency (created_at → finished_at)
   - Min/max task latency

3. **Resource Usage**
   - Scheduler RSS memory (initial and final)
   - Scheduler CPU percentage
   - Peak worker process count

## Usage

### Predefined Configurations

All configs are stored in `perf-configs/*.yaml` and can be customized.

**Quick smoke test** - verify basic functionality:
```bash
just perf-quick
# 10 runs, 5 tasks/run, 0.1s duration
```

**Standard load test** - typical workload:
```bash
just perf-standard
# 100 runs, 5 tasks/run, 0.1s duration
```

**Heavy load test** - stress test:
```bash
just perf-heavy
# 1000 runs, 10 tasks/run, 0.05s duration
```

**Latency test** - measure scheduling overhead:
```bash
just perf-latency
# 50 runs, 5 tasks/run, 0.01s duration (very short tasks)
```

**Memory test** - measure resource usage:
```bash
just perf-memory
# 100 runs, 20 tasks/run, 2.0s duration (long-running tasks)
```

**Run all tests** - execute all configs and save results:
```bash
just perf-all
# Runs all YAML configs in perf-configs/
# Saves results to perf-results/<timestamp>-<config>.txt
```

### Custom Test

Use CLI args for ad-hoc tests:
```bash
just perf <runs> <tasks_per_run> <duration>

# Examples:
just perf 100 5 0.1      # 100 runs, 5 tasks each, 0.1s duration
just perf 50 10 0.5      # 50 runs, 10 tasks each, 0.5s duration
```

### Understanding the Output

```
=== Performance Test Configuration ===
Config: standard
Runs to trigger: 100
Tasks per run: 5
Task duration: 0.1s
Total tasks: 500

Progress: 500/500 tasks | Scheduler: 8960 KB RSS, 5.8% CPU | Workers: 20

=== Performance Test Results ===

Latency Stats:
  Tasks: 500
  Avg latency: 0.156s
  Min latency: 0.112s
  Max latency: 2.345s

Throughput:
  Run submission: 245.32 runs/sec
  Task completion: 89.45 tasks/sec
  Total duration: 5.59s

Explanation:
  - 100 runs created (5 tasks each = 500 total tasks)
  - Run submission measures how fast we enqueue work
  - Task completion measures actual work throughput

Resource Usage:
  Scheduler RSS: 8960 KB
  Scheduler RSS growth: 320 KB
  Peak worker count: 20
```

### Creating Custom Configurations

Create a new YAML file in `perf-configs/`:

```yaml
# perf-configs/my-test.yaml
workflows: 200
tasks_per_workflow: 8
duration: 0.25
```

Then run with:
```bash
cargo run --release --bin perf-test -- --config my-test
```

Or add a justfile command for it.

### Prerequisites

- PostgreSQL running (`just db-start`)
- Database initialized (`just db-init`)
- Rust toolchain (cargo) installed

## Implementation Summary

The performance testing infrastructure includes:

**Predefined test configs** (in `perf-configs/`):
1. `quick.yaml` - Quick smoke test (10 runs × 5 tasks)
2. `standard.yaml` - Standard load test (100 runs × 5 tasks)
3. `heavy.yaml` - Heavy stress test (1000 runs × 10 tasks)
4. `latency.yaml` - Latency measurement (50 runs × 5 tasks, 0.01s duration)
5. `memory.yaml` - Memory test (100 runs × 20 tasks, 2.0s duration)

**Just commands** (all in `perf` group):
- `just perf-quick/standard/heavy/latency/memory` - Run predefined configs
- `just perf-all` - Run all configs and save results to `perf-results/`
- `just perf <runs> <tasks> <duration>` - Custom parameters
- `just clean-data` - Clean database between tests

**Implementation details**:
- Rust binary at `src/bin/perf-test.rs`
- Uses `sqlx` for direct database queries (no SQL parsing)
- Uses `sysinfo` crate for accurate process resource monitoring
- Uses Tokio for async concurrent run triggering
- YAML configs parsed with `serde_yaml`

