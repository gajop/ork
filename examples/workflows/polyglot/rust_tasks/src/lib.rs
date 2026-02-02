//! Library executor task using ork-sdk-rust
//!
//! This demonstrates the HIGH-PERFORMANCE library approach:
//! - No subprocess overhead
//! - Direct FFI calls
//! - Fastest execution method

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TaskInput {
    pub upstream: Upstream,
}

#[derive(Debug, Deserialize)]
pub struct Upstream {
    pub py_generate: NumbersOutput,
}

#[derive(Debug, Deserialize)]
pub struct NumbersOutput {
    pub numbers: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct TaskOutput {
    pub sum: i32,
    pub doubled: Vec<i32>,
    pub method: String,
}

pub fn process_via_library(input: TaskInput) -> TaskOutput {
    let numbers = &input.upstream.py_generate.numbers;
    let sum: i32 = numbers.iter().sum();
    let doubled: Vec<i32> = numbers.iter().map(|n| n * 2).collect();

    TaskOutput {
        sum,
        doubled,
        method: "library_executor".to_string(),
    }
}

// Export C ABI for library executor
ork_sdk_rust::ork_task_library!(process_via_library);
