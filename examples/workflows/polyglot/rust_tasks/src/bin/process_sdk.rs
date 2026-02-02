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
    upstream: Upstream,
}

#[derive(Debug, Deserialize)]
struct Upstream {
    rust_library: LibraryOutput,
}

#[derive(Debug, Deserialize)]
struct LibraryOutput {
    sum: i32,
    doubled: Vec<i32>,
}

#[derive(Debug, Serialize)]
struct TaskOutput {
    total: i32,
    tripled: Vec<i32>,
    method: String,
}

fn process_with_sdk(input: TaskInput) -> TaskOutput {
    eprintln!("[Rust SDK] Processing with SDK helpers...");

    let doubled = &input.upstream.rust_library.doubled;
    let sum = input.upstream.rust_library.sum;

    eprintln!("[Rust SDK] Received sum: {}, doubled: {:?}", sum, doubled);

    let total = sum * 3;
    let tripled: Vec<i32> = doubled.iter().map(|n| n + n / 2).collect();  // 1.5x the doubled values

    eprintln!("[Rust SDK] Total: {}, Tripled: {:?}", total, tripled);

    TaskOutput {
        total,
        tripled,
        method: "process_sdk".to_string(),
    }
}

fn main() {
    // SDK handles all the boilerplate!
    run_task(process_with_sdk);
}
