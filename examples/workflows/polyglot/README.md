# Polyglot Workflow Example

This example demonstrates **ALL 4 Ork execution methods** for polyglot task orchestration.

## The Four Execution Methods

### 1. **Python** (Dynamic Module Loading)
```yaml
py_generate:
  executor: python
  file: python_tasks/tasks.py
  function: generate_numbers
```

**Features:**
- Zero boilerplate - just regular Python functions
- Dynamic module loading and introspection
- Automatic argument passing and type checking
- Python gets special treatment - the easiest way to write tasks

**Use when:** Writing data processing, scripting, or ML tasks in Python

---

### 2. **Library Executor** (High Performance, Rust)
```yaml
rust_library:
  executor: library
  file: rust_tasks/target/debug/libpolyglot_rust_lib.so
```

**Features:**
- Zero subprocess overhead - direct FFI calls
- Uses C ABI for universal language support
- `ork_task_library!` macro handles all boilerplate
- Fastest execution method

**Code example:**
```rust
use ork_sdk_rust::ork_task_library;

fn my_task(input: MyInput) -> MyOutput {
    // Your logic here
}

ork_task_library!(my_task);  // Exports C ABI
```

**Use when:** CPU-intensive tasks, hot paths, or when performance is critical

---

### 3. **Process Executor with SDK** (Clean, Minimal Boilerplate)
```yaml
rust_process_sdk:
  executor: process
  command: cargo run --bin process_sdk
```

**Features:**
- `run_task()` helper handles stdin/stdout
- Automatic `ORK_OUTPUT:` prefix
- Clean, readable code
- Best balance of simplicity and flexibility

**Code example:**
```rust
use ork_sdk_rust::run_task;

fn my_task(input: MyInput) -> MyOutput {
    // Your logic here
}

fn main() {
    run_task(my_task);  // That's it!
}
```

**Use when:** Writing Rust tasks that don't need extreme performance, or any language with SDK support

---

### 4. **Process Executor Raw** (Maximum Control)
```yaml
rust_process_raw:
  executor: process
  command: cargo run --bin process_raw
```

**Features:**
- Manual env var parsing (`ORK_UPSTREAM_JSON`)
- Manual output formatting (`ORK_OUTPUT:`)
- Maximum control over everything
- Shows what happens under the hood

**Code example:**
```rust
fn main() {
    let upstream_json = env::var("ORK_UPSTREAM_JSON").expect(...);
    let upstream: UpstreamData = serde_json::from_str(&upstream_json).expect(...);

    // Your logic

    println!("ORK_OUTPUT:{}", serde_json::to_string(&output).expect(...));
}
```

**Use when:** You need maximum control, custom logic, or are using a language without SDK support

---

## Task Flow

```
┌─────────────┐
│  Python     │ Method 1: Dynamic module loading
│  Generate   │ → Generates list of numbers
└──────┬──────┘
       │
       ├────────────────────┐
       │                    │
       ▼                    ▼
┌─────────────┐      ┌─────────────┐
│  Rust       │      │  Rust       │ Method 2: Library (FFI)
│  Library    │      │  Process    │ → Sum and double numbers
│             │      │  Raw        │ Method 4: Process (raw)
└──────┬──────┘      └─────────────┘ → Same logic, different approach
       │
       ▼
┌─────────────┐
│  Rust       │ Method 3: Process with SDK
│  Process    │ → Further process results
│  SDK        │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Python     │ Method 1 again
│  Format     │ → Format final result
└─────────────┘
```

## Structure

```
polyglot/
├── python_tasks/
│   ├── pyproject.toml
│   └── tasks.py              # Python task functions
├── rust_tasks/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs            # Library executor (cdylib)
│   │   └── bin/
│   │       ├── process_sdk.rs   # Process with SDK
│   │       └── process_raw.rs   # Process raw
│   └── target/debug/
│       └── libpolyglot_rust_lib.so  # Built library
├── polyglot.yaml             # Workflow definition
└── README.md                 # This file
```

## Running the Example

```bash
# Build Rust tasks first
cd examples/workflows/polyglot
cargo build --manifest-path rust_tasks/Cargo.toml

# Run the workflow
just example polyglot
```

## Comparison Table

| Method | Overhead | Boilerplate | Performance | Type Safety | Use Case |
|--------|----------|-------------|-------------|-------------|----------|
| Python | None | None | Good | Runtime | General tasks, data processing |
| Library | None | Minimal (macro) | **Excellent** | Compile-time | CPU-intensive, hot paths |
| Process+SDK | Low | Minimal (1 line) | Good | Compile-time | General compiled tasks |
| Process Raw | Low | High (manual) | Good | Compile-time | Custom logic, debugging |

## Key Takeaways

1. **Python is special** - It gets the best developer experience with zero boilerplate
2. **Library executor** - Use for maximum performance when every millisecond counts
3. **Process with SDK** - Best for most compiled language tasks
4. **Process raw** - Understand this to know how Ork works under the hood

## Adding More Languages

This architecture supports any language:

- **Go**: Can use library executor (via CGO) or process executor
- **Node.js**: Process executor (easy to add SDK)
- **Ruby/Perl/etc**: Process executor
- **C/C++**: Library executor (native C ABI) or process executor
- **Zig**: Library executor or process executor

The key is: **One language gets special treatment (Python), all others have two options (process or library)**.
