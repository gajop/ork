//! Process executor with ork-sdk-rust helpers
//!
//! This demonstrates the SDK-ASSISTED approach:
//! - Clean, minimal boilerplate
//! - Automatic stdin/stdout handling
//! - Automatic ORK_OUTPUT prefix
//! - Best balance of simplicity and flexibility

use ork_sdk_rust::run_task;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct TaskInput {
    rust_library: LibraryOutput,
}

#[derive(Debug, Deserialize)]
struct LibraryOutput {
    sum: i32,
    tripled: Vec<i32>,
}

#[derive(Debug, Serialize)]
struct TaskOutput {
    total: i32,
    tripled: Vec<i32>,
}

fn process_with_sdk(input: TaskInput) -> TaskOutput {
    eprintln!("[Rust SDK] Processing with SDK helpers...");

    let tripled = &input.rust_library.tripled;
    let sum = input.rust_library.sum;

    eprintln!("[Rust SDK] Received sum: {}, tripled: {:?}", sum, tripled);

    let total = sum * 3;
    let tripled_copy = tripled.clone();

    eprintln!("[Rust SDK] Total: {}, Tripled: {:?}", total, tripled_copy);

    TaskOutput {
        total,
        tripled: tripled_copy,
    }
}

fn main() {
    // SDK handles all the boilerplate!
    run_task(process_with_sdk);
}
