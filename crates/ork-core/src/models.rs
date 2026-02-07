// Database-backed orchestration models with event-driven architecture support
// These models include rich metadata for orchestration (timestamps, execution tracking, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "sqlx")]
use sqlx::types::Json;

// Type alias for JSON fields - when sqlx is enabled use sqlx::Json, otherwise use direct Value
#[cfg(feature = "sqlx")]
pub type JsonValue = Json<serde_json::Value>;

#[cfg(not(feature = "sqlx"))]
pub type JsonValue = serde_json::Value;

// Helper to access inner JSON value uniformly
#[cfg(feature = "sqlx")]
#[inline]
pub fn json_inner(j: &JsonValue) -> &serde_json::Value {
    &j.0
}

#[cfg(not(feature = "sqlx"))]
#[inline]
pub fn json_inner(j: &JsonValue) -> &serde_json::Value {
    j
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub job_name: String,
    pub region: String,
    pub project: String,
    pub executor_type: String,
    pub task_params: Option<JsonValue>,
    pub schedule: Option<String>,
    pub schedule_enabled: bool,
    pub last_scheduled_at: Option<DateTime<Utc>>,
    pub next_scheduled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorType {
    CloudRun,
    Process,
    Python,
    Library,
}

impl ExecutorType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cloudrun" => Some(Self::CloudRun),
            "process" => Some(Self::Process),
            "python" => Some(Self::Python),
            "library" => Some(Self::Library),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CloudRun => "cloudrun",
            Self::Process => "process",
            Self::Python => "python",
            Self::Library => "library",
        }
    }
}

impl std::str::FromStr for ExecutorType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Paused,
    Success,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Pending,
    Running,
    Paused,
    Success,
    Failed,
}

impl RunStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Run {
    pub id: Uuid,
    pub workflow_id: Uuid,
    status: String,
    pub triggered_by: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Run {
    pub fn status(&self) -> RunStatus {
        RunStatus::parse(&self.status).unwrap_or(RunStatus::Pending)
    }

    pub fn status_str(&self) -> &str {
        &self.status
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct Task {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub attempts: i32,
    #[serde(default)]
    pub max_retries: i32,
    #[serde(default)]
    pub timeout_seconds: Option<i32>,
    #[serde(default)]
    pub retry_at: Option<DateTime<Utc>>,
    pub execution_name: Option<String>,
    pub params: Option<JsonValue>,
    pub output: Option<JsonValue>,
    pub logs: Option<String>,
    pub error: Option<String>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn status_str(&self) -> &str {
        &self.status
    }
}

/// Combined task and workflow data to avoid N+1 queries
/// Used by scheduler to fetch tasks with their workflow info in a single query
#[derive(Debug, Clone)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct TaskWithWorkflow {
    // Task fields
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    #[allow(dead_code)]
    pub task_status: String,
    pub attempts: i32,
    pub max_retries: i32,
    pub timeout_seconds: Option<i32>,
    pub retry_at: Option<DateTime<Utc>>,
    pub execution_name: Option<String>,
    pub params: Option<JsonValue>,

    // Workflow fields
    pub workflow_id: Uuid,
    #[allow(dead_code)]
    pub job_name: String,
    pub project: String,
    pub region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct WorkflowTask {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub task_index: i32,
    pub task_name: String,
    pub executor_type: String,
    pub depends_on: Vec<String>,
    pub params: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferredJobStatus {
    Pending,
    Polling,
    Completed,
    Failed,
    Cancelled,
}

impl DeferredJobStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "polling" => Some(Self::Polling),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Polling => "polling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for DeferredJobStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(())
    }
}

/// Represents a long-running external job being tracked by the scheduler
/// Used by the Triggerer component to poll external APIs (BigQuery, Cloud Run, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct DeferredJob {
    pub id: Uuid,
    pub task_id: Uuid,
    pub service_type: String,
    pub job_id: String,
    pub job_data: JsonValue,
    status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_polled_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl DeferredJob {
    pub fn status(&self) -> DeferredJobStatus {
        DeferredJobStatus::parse(&self.status).unwrap_or(DeferredJobStatus::Pending)
    }

    pub fn status_str(&self) -> &str {
        &self.status
    }
}
