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
