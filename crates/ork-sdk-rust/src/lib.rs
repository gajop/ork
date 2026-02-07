//! Ork SDK for Rust
//!
//! This crate provides helpers for writing Ork tasks in Rust with minimal boilerplate.
//!
//! # Usage
//!
//! ## For Library Executor (Dynamic Library, High Performance)
//!
//! ```rust,ignore
//! use ork_sdk_rust::{ork_task_library, TaskInput, TaskOutput};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct MyInput {
//!     value: i32,
//! }
//!
//! #[derive(Serialize)]
//! struct MyOutput {
//!     result: i32,
//! }
//!
//! fn my_task(input: MyInput) -> MyOutput {
//!     MyOutput { result: input.value * 2 }
//! }
//!
//! // Export as C ABI for library executor
//! ork_task_library!(my_task);
//! ```
//!
//! Build as `cdylib`:
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//!
//! ## For Process Executor (Binary, Maximum Compatibility)
//!
//! ```rust,ignore
//! use ork_sdk_rust::run_task;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize)]
//! struct MyInput {
//!     value: i32,
//! }
//!
//! #[derive(Serialize)]
//! struct MyOutput {
//!     result: i32,
//! }
//!
//! fn my_task(input: MyInput) -> MyOutput {
//!     MyOutput { result: input.value * 2 }
//! }
//!
//! fn main() {
//!     run_task(my_task);
//! }
//! ```

pub mod deferrables;

use serde::{Deserialize, Serialize};

/// Read task input from environment variables or stdin
///
/// For process executor with upstream: reads from ORK_UPSTREAM_JSON env var
/// For process executor with task_input: reads from ORK_INPUT_JSON env var
/// Fallback: reads from stdin
pub fn read_input<T: for<'de> Deserialize<'de>>() -> Result<T, Box<dyn std::error::Error>> {
    // Try ORK_UPSTREAM_JSON first (for tasks with dependencies)
    if let Ok(upstream_json) = std::env::var("ORK_UPSTREAM_JSON") {
        let value: serde_json::Value = serde_json::from_str(&upstream_json)?;
        // Wrap in task_input structure
        let wrapped = serde_json::json!({"upstream": value});
        return Ok(serde_json::from_value(wrapped)?);
    }

    // Try ORK_INPUT_JSON (for tasks with explicit input)
    if let Ok(input_json) = std::env::var("ORK_INPUT_JSON") {
        return Ok(serde_json::from_str(&input_json)?);
    }

    // Fallback to stdin
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let parsed = serde_json::from_str(&input)?;
    Ok(parsed)
}

/// Write task output to stdout with ORK_OUTPUT: prefix
pub fn write_output<T: Serialize>(output: &T) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(output)?;
    println!("ORK_OUTPUT:{}", json);
    Ok(())
}

/// Run a task function with input from stdin and output to stdout
///
/// This is the main entry point for process executor tasks.
///
/// # Example
///
/// ```rust,ignore
/// fn main() {
///     run_task(my_task_function);
/// }
/// ```
pub fn run_task<I, O, F>(task_fn: F)
where
    I: for<'de> Deserialize<'de>,
    O: Serialize,
    F: FnOnce(I) -> O,
{
    match read_input::<I>() {
        Ok(input) => {
            let output = task_fn(input);
            if let Err(e) = write_output(&output) {
                eprintln!("Error writing output: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error reading input: {}", e);
            std::process::exit(1);
        }
    }
}

/// Helper trait for task inputs
pub trait TaskInput: for<'de> Deserialize<'de> {}
impl<T: for<'de> Deserialize<'de>> TaskInput for T {}

/// Helper trait for task outputs
pub trait TaskOutput: Serialize {}
impl<T: Serialize> TaskOutput for T {}

/// Generate C ABI exports for library executor
///
/// This macro generates the necessary `extern "C"` functions for the library executor.
///
/// # Example
///
/// ```rust,ignore
/// fn process_numbers(input: NumbersInput) -> ProcessedOutput {
///     // Your logic here
/// }
///
/// ork_task_library!(process_numbers);
/// ```
#[macro_export]
macro_rules! ork_task_library {
    ($task_fn:ident) => {
        use std::ffi::{CStr, CString};
        use std::os::raw::c_char;

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn ork_task_run(input_ptr: *const c_char) -> *mut c_char {
            if input_ptr.is_null() {
                let error = CString::new(r#"{"error": "null input pointer"}"#).unwrap();
                return error.into_raw();
            }

            let input_str = unsafe {
                match CStr::from_ptr(input_ptr).to_str() {
                    Ok(s) => s,
                    Err(e) => {
                        let error = CString::new(format!(r#"{{"error": "invalid UTF-8: {}"}}"#, e))
                            .unwrap();
                        return error.into_raw();
                    }
                }
            };

            let input = match serde_json::from_str(input_str) {
                Ok(i) => i,
                Err(e) => {
                    let error =
                        CString::new(format!(r#"{{"error": "JSON parse error: {}"}}"#, e)).unwrap();
                    return error.into_raw();
                }
            };

            let output = $task_fn(input);

            let output_json = match serde_json::to_string(&output) {
                Ok(j) => format!("ORK_OUTPUT:{}", j),
                Err(e) => {
                    let error = format!(r#"{{"error": "JSON serialize error: {}"}}"#, e);
                    return CString::new(error).unwrap().into_raw();
                }
            };

            CString::new(output_json).unwrap().into_raw()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn ork_task_free(ptr: *mut c_char) {
            if !ptr.is_null() {
                unsafe {
                    let _ = CString::from_raw(ptr);
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct TestInput {
        value: i32,
    }

    #[derive(Serialize)]
    struct TestOutput {
        result: i32,
    }

    fn test_task(input: TestInput) -> TestOutput {
        TestOutput {
            result: input.value * 2,
        }
    }

    #[test]
    fn test_task_output_trait() {
        let output = TestOutput { result: 42 };
        let _: &dyn TaskOutput = &output;
    }

    #[test]
    fn test_task_input_trait() {
        fn accepts_input<T: TaskInput>(_: T) {}
        let input = TestInput { value: 21 };
        accepts_input(input);
    }
}
