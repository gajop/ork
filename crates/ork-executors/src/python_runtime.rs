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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_get_run_task_script_writes_embedded_content() {
        let _guard = env_lock().lock().expect("env lock");
        let temp_root =
            std::env::temp_dir().join(format!("ork-runtime-test-{}", uuid::Uuid::new_v4()));
        let prev = std::env::var("XDG_CACHE_HOME").ok();

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &temp_root);
        }

        let path = get_run_task_script().expect("get run_task.py");
        assert!(path.exists());
        let content = fs::read_to_string(&path).expect("read script");
        assert_eq!(content, RUN_TASK_PY);

        match prev {
            Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
        }
        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn test_get_inspect_task_script_writes_embedded_content() {
        let _guard = env_lock().lock().expect("env lock");
        let temp_root =
            std::env::temp_dir().join(format!("ork-runtime-test-{}", uuid::Uuid::new_v4()));
        let prev = std::env::var("XDG_CACHE_HOME").ok();

        unsafe {
            std::env::set_var("XDG_CACHE_HOME", &temp_root);
        }

        let path = get_inspect_task_script().expect("get inspect_task.py");
        assert!(path.exists());
        let content = fs::read_to_string(&path).expect("read script");
        assert_eq!(content, INSPECT_TASK_PY);

        let second = get_inspect_task_script().expect("get inspect_task.py again");
        assert_eq!(second, path);

        match prev {
            Some(v) => unsafe { std::env::set_var("XDG_CACHE_HOME", v) },
            None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
        }
        let _ = fs::remove_dir_all(temp_root);
    }
}
