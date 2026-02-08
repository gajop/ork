use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use tokio::time::{Duration, sleep};

#[derive(Args)]
pub struct RunWorkflow {
    /// Path to workflow YAML file
    #[arg(short, long)]
    pub file: String,

    /// Base API URL (defaults to ORK_API_URL or http://127.0.0.1:4000)
    #[arg(long, default_value = "http://127.0.0.1:4000")]
    pub api_url: String,

    /// Project label for the workflow (default: local)
    #[arg(long, default_value = "local")]
    pub project: String,

    /// Region label for the workflow (default: local)
    #[arg(long, default_value = "local")]
    pub region: String,

    /// Root directory for resolving task paths/modules (defaults to the YAML's parent)
    #[arg(long)]
    pub root: Option<String>,

    /// Replace workflow tasks for an existing workflow (runs are preserved)
    #[arg(long, default_value_t = true)]
    pub replace: bool,
}

fn extract_run_status(data: &serde_json::Value) -> Result<&str> {
    let status = data
        .get("run")
        .and_then(|r| r.get("status"))
        .and_then(|s| s.as_str())
        .unwrap_or("");
    if status.is_empty() {
        anyhow::bail!("Run status not found in API response");
    }
    Ok(status)
}

impl RunWorkflow {
    pub async fn execute(self) -> Result<()> {
        let api_url = std::env::var("ORK_API_URL").unwrap_or(self.api_url);
        let file_path = PathBuf::from(&self.file);
        let yaml = std::fs::read_to_string(&file_path)
            .with_context(|| format!("Missing workflow file: {}", self.file))?;
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
        let workflow_name = yaml_value
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' in {}", self.file))?
            .to_string();

        let root = self
            .root
            .map(PathBuf::from)
            .or_else(|| file_path.parent().map(|p| p.to_path_buf()))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let root = root.canonicalize().unwrap_or(root);

        let payload = serde_json::json!({
            "yaml": yaml,
            "project": self.project,
            "region": self.region,
            "root": root.to_string_lossy(),
            "replace": self.replace,
        });

        let client = reqwest::Client::new();
        let create_resp = client
            .post(format!("{}/api/workflows", api_url))
            .json(&payload)
            .send()
            .await?;

        if !create_resp.status().is_success() {
            let body = create_resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create workflow: {}", body);
        }

        #[derive(serde::Deserialize)]
        struct TriggerResp {
            run_id: String,
        }

        let trigger_resp = client
            .post(format!("{}/api/runs", api_url))
            .json(&serde_json::json!({ "workflow": workflow_name }))
            .send()
            .await?;

        if !trigger_resp.status().is_success() {
            let body = trigger_resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to trigger workflow: {}", body);
        }

        let trigger: TriggerResp = trigger_resp.json().await?;
        let run_id = trigger.run_id;
        if run_id.is_empty() {
            anyhow::bail!("API did not return a run_id");
        }
        println!("✓ Triggered workflow run: {}", run_id);

        loop {
            let run_resp = client
                .get(format!("{}/api/runs/{}", api_url, run_id))
                .send()
                .await?;
            if !run_resp.status().is_success() {
                let body = run_resp.text().await.unwrap_or_default();
                anyhow::bail!("Failed to read run status: {}", body);
            }
            let data: serde_json::Value = run_resp.json().await?;
            let status = extract_run_status(&data)?;
            println!("Run {} status: {}", run_id, status);
            if matches!(status, "success" | "failed" | "cancelled") {
                break;
            }
            sleep(Duration::from_secs(2)).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Path,
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
    };
    use std::net::SocketAddr;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use uuid::Uuid;

    fn write_temp_workflow_with_name(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("ork-run-workflow-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            format!(
                r#"
name: {}
tasks:
  a:
    executor: process
    command: "echo hi"
"#,
                name
            ),
        )
        .expect("write temp workflow");
        path
    }

    #[tokio::test]
    async fn test_run_workflow_missing_file_errors() {
        let cmd = RunWorkflow {
            file: "/tmp/does-not-exist-workflow.yaml".to_string(),
            api_url: "http://127.0.0.1:4000".to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_workflow_missing_name_field_errors() {
        let path = std::env::temp_dir().join(format!("ork-run-workflow-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"
tasks:
  a:
    executor: process
    command: "echo hi"
"#,
        )
        .expect("write temp workflow");

        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: "http://127.0.0.1:4000".to_string(),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        assert!(result.is_err());

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_run_workflow_success_with_mock_api() {
        async fn create_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"ok": true}))
        }

        async fn trigger_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"run_id": "run-1"}))
        }

        async fn run_status_handler(Path(_run_id): Path<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "run": { "status": "success" }
            }))
        }

        let app = Router::new()
            .route("/api/workflows", post(create_handler))
            .route("/api/runs", post(trigger_handler))
            .route("/api/runs/{run_id}", get(run_status_handler));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = std::env::temp_dir().join(format!("ork-run-workflow-{}.yaml", Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"
name: wf-api
tasks:
  a:
    executor: process
    command: "echo hi"
"#,
        )
        .expect("write temp workflow");

        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_workflow_create_failure_returns_api_error() {
        async fn create_handler() -> impl IntoResponse {
            (StatusCode::INTERNAL_SERVER_ERROR, "create failed")
        }

        let app = Router::new().route("/api/workflows", post(create_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = write_temp_workflow_with_name("wf-create-fail");
        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        let err = result.expect_err("create should fail");
        assert!(err.to_string().contains("Failed to create workflow"));
        assert!(err.to_string().contains("create failed"));
    }

    #[tokio::test]
    async fn test_run_workflow_trigger_failure_returns_api_error() {
        async fn create_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"ok": true}))
        }

        async fn trigger_handler() -> impl IntoResponse {
            (StatusCode::INTERNAL_SERVER_ERROR, "trigger failed")
        }

        let app = Router::new()
            .route("/api/workflows", post(create_handler))
            .route("/api/runs", post(trigger_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = write_temp_workflow_with_name("wf-trigger-fail");
        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        let err = result.expect_err("trigger should fail");
        assert!(err.to_string().contains("Failed to trigger workflow"));
        assert!(err.to_string().contains("trigger failed"));
    }

    #[tokio::test]
    async fn test_run_workflow_errors_when_api_returns_empty_run_id() {
        async fn create_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"ok": true}))
        }

        async fn trigger_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"run_id": ""}))
        }

        let app = Router::new()
            .route("/api/workflows", post(create_handler))
            .route("/api/runs", post(trigger_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = write_temp_workflow_with_name("wf-empty-run-id");
        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        let err = result.expect_err("empty run_id should fail");
        assert!(err.to_string().contains("did not return a run_id"));
    }

    #[tokio::test]
    async fn test_run_workflow_errors_when_run_status_endpoint_fails() {
        async fn create_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"ok": true}))
        }

        async fn trigger_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"run_id": "run-err"}))
        }

        async fn run_status_handler(Path(_run_id): Path<String>) -> impl IntoResponse {
            (StatusCode::INTERNAL_SERVER_ERROR, "status failed")
        }

        let app = Router::new()
            .route("/api/workflows", post(create_handler))
            .route("/api/runs", post(trigger_handler))
            .route("/api/runs/{run_id}", get(run_status_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = write_temp_workflow_with_name("wf-run-status-fail");
        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        let err = result.expect_err("run status endpoint error should fail");
        assert!(err.to_string().contains("Failed to read run status"));
        assert!(err.to_string().contains("status failed"));
    }

    #[tokio::test]
    async fn test_run_workflow_polls_until_success() {
        async fn create_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"ok": true}))
        }

        async fn trigger_handler() -> Json<serde_json::Value> {
            Json(serde_json::json!({"run_id": "run-poll"}))
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_handler = Arc::clone(&calls);
        let run_status_handler = move |Path(_run_id): Path<String>| {
            let calls_handler = Arc::clone(&calls_handler);
            async move {
                let call = calls_handler.fetch_add(1, Ordering::SeqCst);
                if call == 0 {
                    Json(serde_json::json!({
                        "run": { "status": "running" }
                    }))
                } else {
                    Json(serde_json::json!({
                        "run": { "status": "success" }
                    }))
                }
            }
        };

        let app = Router::new()
            .route("/api/workflows", post(create_handler))
            .route("/api/runs", post(trigger_handler))
            .route("/api/runs/{run_id}", get(run_status_handler));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr: SocketAddr = listener.local_addr().expect("get listener addr");
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock api");
        });

        let path = write_temp_workflow_with_name("wf-env-url");
        let cmd = RunWorkflow {
            file: path.to_string_lossy().to_string(),
            api_url: format!("http://{}", addr),
            project: "local".to_string(),
            region: "local".to_string(),
            root: None,
            replace: true,
        };

        let result = cmd.execute().await;
        server.abort();
        let _ = std::fs::remove_file(path);

        assert!(result.is_ok());
        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "expected at least two polling calls"
        );
    }

    #[test]
    fn test_extract_run_status_valid_and_missing() {
        let ok = serde_json::json!({"run":{"status":"success"}});
        assert_eq!(extract_run_status(&ok).expect("status"), "success");

        let missing = serde_json::json!({"run":{}});
        let err = extract_run_status(&missing).expect_err("missing status should error");
        assert!(err.to_string().contains("Run status not found"));
    }
}
