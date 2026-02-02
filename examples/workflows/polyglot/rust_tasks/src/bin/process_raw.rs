//! Raw process executor (no SDK)
//!
//! This demonstrates the RAW approach:
//! - Manual env var parsing
//! - Manual JSON serialization
//! - Manual ORK_OUTPUT prefix
//! - Maximum control, more boilerplate

use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize)]
struct UpstreamData {
    py_generate: NumbersOutput,
}

#[derive(Debug, Deserialize)]
struct NumbersOutput {
    numbers: Vec<i32>,
}

#[derive(Debug, Serialize)]
struct ProcessedOutput {
    sum: i32,
    doubled: Vec<i32>,
    method: String,
}

fn main() {
    eprintln!("[Rust Raw] Starting number processing...");

    // Read upstream data from environment variable (manual)
    let upstream_json = env::var("ORK_UPSTREAM_JSON")
        .expect("ORK_UPSTREAM_JSON not set");

    eprintln!("[Rust Raw] Received upstream: {}", upstream_json);

    let upstream: UpstreamData = serde_json::from_str(&upstream_json)
        .expect("Failed to parse upstream JSON");

    let numbers = &upstream.py_generate.numbers;
    eprintln!("[Rust Raw] Parsed {} numbers", numbers.len());

    // Process the numbers
    let sum: i32 = numbers.iter().sum();
    let doubled: Vec<i32> = numbers.iter().map(|n| n * 2).collect();

    eprintln!("[Rust Raw] Sum: {}, Doubled: {:?}", sum, doubled);

    let output = ProcessedOutput {
        sum,
        doubled,
        method: "process_raw".to_string(),
    };

    // Manual output with ORK_OUTPUT prefix
    let json = serde_json::to_string(&output).expect("Failed to serialize output");
    println!("ORK_OUTPUT:{}", json);

    eprintln!("[Rust Raw] Processing complete!");
}
