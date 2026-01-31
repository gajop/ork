use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

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
            let status = data
                .get("run")
                .and_then(|r| r.get("status"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if status.is_empty() {
                anyhow::bail!("Run status not found in API response");
            }
            println!("Run {} status: {}", run_id, status);
            if matches!(status, "success" | "failed" | "cancelled") {
                break;
            }
            sleep(Duration::from_secs(2)).await;
        }

        Ok(())
    }
}
