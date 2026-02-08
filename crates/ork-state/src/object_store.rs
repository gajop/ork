use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use ork_core::error::{OrkError, OrkResult};
use ork_core::types::{TaskSpec, TaskStatusFile};

#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn write_spec(&self, spec: &TaskSpec) -> OrkResult<PathBuf>;
    async fn read_spec(&self, run_id: &str, task_name: &str) -> OrkResult<TaskSpec>;
    fn spec_path(&self, run_id: &str, task_name: &str) -> PathBuf;
    fn task_dir(&self, run_id: &str, task_name: &str) -> PathBuf;
    fn output_path(&self, run_id: &str, task_name: &str) -> PathBuf;

    async fn write_status(
        &self,
        run_id: &str,
        task_name: &str,
        status: &TaskStatusFile,
    ) -> OrkResult<()>;
    async fn read_status(&self, run_id: &str, task_name: &str)
    -> OrkResult<Option<TaskStatusFile>>;

    async fn write_output(
        &self,
        run_id: &str,
        task_name: &str,
        output: &serde_json::Value,
    ) -> OrkResult<()>;
    async fn read_output(
        &self,
        run_id: &str,
        task_name: &str,
    ) -> OrkResult<Option<serde_json::Value>>;
}

#[derive(Debug, Clone)]
pub struct LocalObjectStore {
    base: PathBuf,
}

impl LocalObjectStore {
    pub fn new(base: impl AsRef<Path>) -> Self {
        Self {
            base: base.as_ref().to_path_buf(),
        }
    }

    pub fn task_dir(&self, run_id: &str, task_name: &str) -> PathBuf {
        self.base.join(run_id).join(task_name)
    }

    pub fn spec_path(&self, run_id: &str, task_name: &str) -> PathBuf {
        self.task_dir(run_id, task_name).join("spec.json")
    }

    pub fn status_path(&self, run_id: &str, task_name: &str) -> PathBuf {
        self.task_dir(run_id, task_name).join("status.json")
    }

    pub fn output_path(&self, run_id: &str, task_name: &str) -> PathBuf {
        self.task_dir(run_id, task_name).join("output.json")
    }
}

#[async_trait]
impl ObjectStore for LocalObjectStore {
    fn spec_path(&self, run_id: &str, task_name: &str) -> PathBuf {
        LocalObjectStore::spec_path(self, run_id, task_name)
    }

    fn task_dir(&self, run_id: &str, task_name: &str) -> PathBuf {
        LocalObjectStore::task_dir(self, run_id, task_name)
    }

    fn output_path(&self, run_id: &str, task_name: &str) -> PathBuf {
        LocalObjectStore::output_path(self, run_id, task_name)
    }

    async fn write_spec(&self, spec: &TaskSpec) -> OrkResult<PathBuf> {
        let path = self.spec_path(&spec.run_id, &spec.task_name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .await
                .map_err(|source| OrkError::WriteFile {
                    path: dir.to_path_buf(),
                    source,
                })?;
        }

        let bytes = serde_json::to_vec_pretty(spec).map_err(|source| OrkError::JsonParse {
            path: path.clone(),
            source,
        })?;

        fs::write(&path, bytes)
            .await
            .map_err(|source| OrkError::WriteFile {
                path: path.clone(),
                source,
            })?;

        Ok(path)
    }

    async fn read_spec(&self, run_id: &str, task_name: &str) -> OrkResult<TaskSpec> {
        let path = self.spec_path(run_id, task_name);
        let data = fs::read(&path).await.map_err(|source| OrkError::ReadFile {
            path: path.clone(),
            source,
        })?;
        serde_json::from_slice(&data).map_err(|source| OrkError::JsonParse { path, source })
    }

    async fn write_status(
        &self,
        run_id: &str,
        task_name: &str,
        status: &TaskStatusFile,
    ) -> OrkResult<()> {
        let path = self.status_path(run_id, task_name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .await
                .map_err(|source| OrkError::WriteFile {
                    path: dir.to_path_buf(),
                    source,
                })?;
        }

        let bytes = serde_json::to_vec_pretty(status).map_err(|source| OrkError::JsonParse {
            path: path.clone(),
            source,
        })?;
        fs::write(&path, bytes)
            .await
            .map_err(|source| OrkError::WriteFile {
                path: path.clone(),
                source,
            })?;
        Ok(())
    }

    async fn read_status(
        &self,
        run_id: &str,
        task_name: &str,
    ) -> OrkResult<Option<TaskStatusFile>> {
        let path = self.status_path(run_id, task_name);
        match fs::read(&path).await {
            Ok(bytes) => {
                let parsed: TaskStatusFile =
                    serde_json::from_slice(&bytes).map_err(|source| OrkError::JsonParse {
                        path: path.clone(),
                        source,
                    })?;
                Ok(Some(parsed))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(OrkError::ReadFile { path, source }),
        }
    }

    async fn write_output(
        &self,
        run_id: &str,
        task_name: &str,
        output: &serde_json::Value,
    ) -> OrkResult<()> {
        let path = self.output_path(run_id, task_name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .await
                .map_err(|source| OrkError::WriteFile {
                    path: dir.to_path_buf(),
                    source,
                })?;
        }
        let bytes = serde_json::to_vec_pretty(output).map_err(|source| OrkError::JsonParse {
            path: path.clone(),
            source,
        })?;
        fs::write(&path, bytes)
            .await
            .map_err(|source| OrkError::WriteFile {
                path: path.clone(),
                source,
            })?;
        Ok(())
    }

    async fn read_output(
        &self,
        run_id: &str,
        task_name: &str,
    ) -> OrkResult<Option<serde_json::Value>> {
        let path = self.output_path(run_id, task_name);
        match fs::read(&path).await {
            Ok(bytes) => {
                let parsed: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|source| OrkError::JsonParse {
                        path: path.clone(),
                        source,
                    })?;
                Ok(Some(parsed))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(OrkError::ReadFile { path, source }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use indexmap::IndexMap;
    use ork_core::types::{TaskSpec, TaskStatus, TaskStatusFile};
    use ork_core::workflow::ExecutorKind;
    use uuid::Uuid;

    fn sample_spec(run_id: &str, task_name: &str) -> TaskSpec {
        TaskSpec {
            run_id: run_id.to_string(),
            workflow_name: "wf".to_string(),
            task_name: task_name.to_string(),
            attempt: 1,
            executor: ExecutorKind::Process,
            input: serde_json::json!({"x": 1}),
            upstream: IndexMap::new(),
        }
    }

    fn temp_store() -> (LocalObjectStore, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("ork-object-store-{}", Uuid::new_v4()));
        (LocalObjectStore::new(&dir), dir)
    }

    #[tokio::test]
    async fn test_write_and_read_spec() {
        let (store, dir) = temp_store();
        let spec = sample_spec("run-1", "task-a");

        let path = store.write_spec(&spec).await.expect("write spec");
        assert!(path.ends_with("spec.json"));

        let loaded = store.read_spec("run-1", "task-a").await.expect("read spec");
        assert_eq!(loaded.task_name, "task-a");
        assert_eq!(loaded.input, serde_json::json!({"x": 1}));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_write_and_read_status() {
        let (store, dir) = temp_store();
        let status = TaskStatusFile {
            status: TaskStatus::Running,
            started_at: Some(Utc::now()),
            finished_at: None,
            heartbeat_at: Some(Utc::now()),
            error: None,
        };

        store
            .write_status("run-1", "task-a", &status)
            .await
            .expect("write status");
        let loaded = store
            .read_status("run-1", "task-a")
            .await
            .expect("read status")
            .expect("status should exist");
        assert_eq!(loaded.status, TaskStatus::Running);

        let missing = store
            .read_status("run-1", "missing")
            .await
            .expect("read missing status");
        assert!(missing.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_write_and_read_output() {
        let (store, dir) = temp_store();
        let output = serde_json::json!({"ok": true, "value": 42});

        store
            .write_output("run-1", "task-a", &output)
            .await
            .expect("write output");
        let loaded = store
            .read_output("run-1", "task-a")
            .await
            .expect("read output")
            .expect("output should exist");
        assert_eq!(loaded["ok"], true);
        assert_eq!(loaded["value"], 42);

        let missing = store
            .read_output("run-1", "missing")
            .await
            .expect("read missing output");
        assert!(missing.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_object_store_path_helpers_and_missing_spec_error() {
        let (store, dir) = temp_store();
        let trait_obj: &dyn ObjectStore = &store;

        let expected_dir = dir.join("run-2").join("task-b");
        assert_eq!(trait_obj.task_dir("run-2", "task-b"), expected_dir);
        assert_eq!(
            trait_obj.spec_path("run-2", "task-b"),
            expected_dir.join("spec.json")
        );
        assert_eq!(
            trait_obj.output_path("run-2", "task-b"),
            expected_dir.join("output.json")
        );

        let err = store
            .read_spec("run-2", "missing")
            .await
            .expect_err("missing spec should error");
        assert!(matches!(err, OrkError::ReadFile { .. }));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_read_spec_status_and_output_invalid_json_errors() {
        let (store, dir) = temp_store();
        let task_dir = store.task_dir("run-3", "task-c");
        tokio::fs::create_dir_all(&task_dir)
            .await
            .expect("create task dir");

        tokio::fs::write(store.spec_path("run-3", "task-c"), b"{not-json")
            .await
            .expect("write invalid spec");
        let spec_err = store
            .read_spec("run-3", "task-c")
            .await
            .expect_err("invalid spec json should error");
        assert!(matches!(spec_err, OrkError::JsonParse { .. }));

        tokio::fs::write(store.status_path("run-3", "task-c"), b"{bad-json")
            .await
            .expect("write invalid status");
        let status_err = store
            .read_status("run-3", "task-c")
            .await
            .expect_err("invalid status json should error");
        assert!(matches!(status_err, OrkError::JsonParse { .. }));

        tokio::fs::write(store.output_path("run-3", "task-c"), b"{also-bad")
            .await
            .expect("write invalid output");
        let output_err = store
            .read_output("run-3", "task-c")
            .await
            .expect_err("invalid output json should error");
        assert!(matches!(output_err, OrkError::JsonParse { .. }));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_write_and_read_error_paths_return_structured_errors() {
        let (store, dir) = temp_store();
        tokio::fs::create_dir_all(&dir)
            .await
            .expect("create temp root");

        let base_file = dir.join("base-file");
        tokio::fs::write(&base_file, b"x")
            .await
            .expect("create base file");
        let bad_store = LocalObjectStore::new(&base_file);
        let spec = sample_spec("run-err", "task-err");
        let write_spec_err = bad_store
            .write_spec(&spec)
            .await
            .expect_err("create_dir_all on file path should fail");
        assert!(matches!(write_spec_err, OrkError::WriteFile { .. }));

        let task_dir = store.task_dir("run-err", "task-err");
        tokio::fs::create_dir_all(&task_dir)
            .await
            .expect("create task dir");
        tokio::fs::create_dir(store.spec_path("run-err", "task-err"))
            .await
            .expect("create spec path dir");
        let write_spec_err = store
            .write_spec(&spec)
            .await
            .expect_err("writing spec to directory path should fail");
        assert!(matches!(write_spec_err, OrkError::WriteFile { .. }));

        let status = TaskStatusFile {
            status: TaskStatus::Running,
            started_at: Some(Utc::now()),
            finished_at: None,
            heartbeat_at: Some(Utc::now()),
            error: None,
        };
        tokio::fs::create_dir(store.status_path("run-err", "task-err"))
            .await
            .expect("create status path dir");
        let write_status_err = store
            .write_status("run-err", "task-err", &status)
            .await
            .expect_err("writing status to directory path should fail");
        assert!(matches!(write_status_err, OrkError::WriteFile { .. }));

        tokio::fs::create_dir(store.output_path("run-err", "task-err"))
            .await
            .expect("create output path dir");
        let write_output_err = store
            .write_output("run-err", "task-err", &serde_json::json!({"ok": true}))
            .await
            .expect_err("writing output to directory path should fail");
        assert!(matches!(write_output_err, OrkError::WriteFile { .. }));

        let read_status_err = store
            .read_status("run-err", "task-err")
            .await
            .expect_err("reading status from directory should fail");
        assert!(matches!(read_status_err, OrkError::ReadFile { .. }));

        let read_output_err = store
            .read_output("run-err", "task-err")
            .await
            .expect_err("reading output from directory should fail");
        assert!(matches!(read_output_err, OrkError::ReadFile { .. }));

        let _ = std::fs::remove_dir_all(dir);
    }
}
