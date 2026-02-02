//! Embedded Python runtime scripts for task execution.
//!
//! These scripts are embedded at compile time and extracted at runtime
//! to avoid external file dependencies.

/// The Python task runner script.
pub const RUN_TASK_PY: &str = include_str!("../runtime/python/run_task.py");

/// The Python task introspection script.
pub const INSPECT_TASK_PY: &str = include_str!("../runtime/python/inspect_task.py");

use std::fs;
use std::io;
use std::path::PathBuf;

/// Get a temporary path to the run_task.py script.
/// Creates the file if it doesn't exist.
pub fn get_run_task_script() -> io::Result<PathBuf> {
    let cache_dir = get_runtime_cache_dir()?;
    let script_path = cache_dir.join("run_task.py");

    if !script_path.exists() {
        fs::write(&script_path, RUN_TASK_PY)?;
    }

    Ok(script_path)
}

/// Get a temporary path to the inspect_task.py script.
/// Creates the file if it doesn't exist.
pub fn get_inspect_task_script() -> io::Result<PathBuf> {
    let cache_dir = get_runtime_cache_dir()?;
    let script_path = cache_dir.join("inspect_task.py");

    if !script_path.exists() {
        fs::write(&script_path, INSPECT_TASK_PY)?;
    }

    Ok(script_path)
}

fn get_runtime_cache_dir() -> io::Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("ork")
        .join("runtime");

    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir)
}
