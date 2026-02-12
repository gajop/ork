//! Library executor task using ork-sdk-rust
//!
//! This demonstrates the HIGH-PERFORMANCE library approach:
//! - No subprocess overhead
//! - Direct FFI calls
//! - Fastest execution method

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TaskInput {
    pub py_generate: NumbersOutput,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NumbersOutput {
    pub numbers: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct TaskOutput {
    pub sum: i32,
    pub tripled: Vec<i32>,
}

pub fn process_via_library(input: TaskInput) -> TaskOutput {
    let numbers = &input.py_generate.numbers;
    let sum: i32 = numbers.iter().sum();
    let tripled: Vec<i32> = numbers.iter().map(|n| n * 3).collect();

    TaskOutput { sum, tripled }
}

// Export C ABI for library executor
ork_sdk_rust::ork_task_library!(process_via_library);
