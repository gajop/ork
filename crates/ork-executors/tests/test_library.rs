//! Tests for library executor
//!
//! These tests verify that the library executor can:
//! - Load dynamic libraries
//! - Call C ABI functions
//! - Handle input/output serialization
//! - Manage memory correctly
//! - Handle errors gracefully

#[cfg(feature = "library")]
mod library_tests {
    use ork_core::executor::{Executor, StatusUpdate};
    use ork_executors::LibraryExecutor;
    use std::path::PathBuf;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    /// Get path to the test library
    fn get_test_library_path() -> PathBuf {
        // The polyglot example library serves as our test library
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let lib_path = PathBuf::from(manifest_dir)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/workflows/polyglot/rust_tasks/target/debug/libpolyglot_rust_lib.so");

        if !lib_path.exists() {
            panic!(
                "Test library not found at {}. Run: cargo build --manifest-path examples/workflows/polyglot/rust_tasks/Cargo.toml",
                lib_path.display()
            );
        }

        lib_path
    }

    #[tokio::test]
    async fn test_library_executor_basic() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = get_test_library_path();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "upstream": {
                "py_generate": {
                    "numbers": [1, 2, 3, 4, 5]
                }
            }
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        // Collect status updates
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        // Should have received status updates
        assert!(!updates.is_empty(), "No status updates received");

        // Find the completed update
        let completed = updates.iter().find(|u| u.status == "completed");
        assert!(completed.is_some(), "No completed status update found");

        let completed = completed.unwrap();
        assert!(completed.output.is_some(), "No output in completed status");

        let output = completed.output.as_ref().unwrap();
        assert!(output.get("sum").is_some(), "Output missing 'sum' field");
        assert!(output.get("doubled").is_some(), "Output missing 'doubled' field");
        assert!(output.get("method").is_some(), "Output missing 'method' field");

        // Verify values
        assert_eq!(output["sum"], 15); // 1+2+3+4+5 = 15
        assert_eq!(output["doubled"], serde_json::json!([2, 4, 6, 8, 10]));
        assert_eq!(output["method"], "library_executor");
    }

    #[tokio::test]
    async fn test_library_executor_missing_path() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, _rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        // No library_path provided
        let params = serde_json::json!({});

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_err(), "Should fail when library_path is missing");
        assert!(
            result.unwrap_err().to_string().contains("library_path"),
            "Error should mention missing library_path"
        );
    }

    #[tokio::test]
    async fn test_library_executor_invalid_path() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let params = serde_json::json!({
            "library_path": "/nonexistent/path/to/library.so"
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_err(), "Should fail when library doesn't exist");

        // Should have received a failed status update
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        let failed = updates.iter().find(|u| u.status == "failed");
        assert!(failed.is_some(), "Should have failed status update");
        assert!(failed.unwrap().error.is_some(), "Failed update should have error message");
    }

    #[tokio::test]
    async fn test_library_executor_with_empty_input() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = get_test_library_path();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "upstream": {
                "py_generate": {
                    "numbers": []
                }
            }
        });

        let result = executor.execute(task_id, "test_task", Some(params)).await;

        assert!(result.is_ok(), "Should handle empty input");

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        let completed = updates.iter().find(|u| u.status == "completed");
        assert!(completed.is_some());

        let output = completed.unwrap().output.as_ref().unwrap();
        assert_eq!(output["sum"], 0);
        assert_eq!(output["doubled"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_library_executor_status_updates() {
        let executor = LibraryExecutor::new();
        let task_id = Uuid::new_v4();
        let (tx, mut rx) = mpsc::unbounded_channel();

        executor.set_status_channel(tx).await;

        let lib_path = get_test_library_path();
        let params = serde_json::json!({
            "library_path": lib_path.to_string_lossy(),
            "upstream": {
                "py_generate": {
                    "numbers": [10, 20, 30]
                }
            }
        });

        let _result = executor.execute(task_id, "test_task", Some(params)).await;

        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }

        // Should have at least running and completed updates
        assert!(updates.len() >= 2, "Should have multiple status updates");

        // First update should be running
        assert_eq!(updates[0].status, "running");
        assert_eq!(updates[0].task_id, task_id);
        assert!(updates[0].log.is_some());

        // Last update should be completed
        let last = updates.last().unwrap();
        assert_eq!(last.status, "completed");
        assert_eq!(last.task_id, task_id);
    }
}
