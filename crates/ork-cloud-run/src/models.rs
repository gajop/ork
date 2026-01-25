use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub job_name: String,
    pub region: String,
    pub project: String,
    pub executor_type: String,
    pub task_params: Option<Json<serde_json::Value>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorType {
    CloudRun,
    Process,
}

impl ExecutorType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cloudrun" => Some(Self::CloudRun),
            "process" => Some(Self::Process),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CloudRun => "cloudrun",
            Self::Process => "process",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Dispatched,
    Running,
    Success,
    Failed,
}

impl TaskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "dispatched" => Some(Self::Dispatched),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Dispatched => "dispatched",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RunStatus {
    Pending,
    Running,
    Success,
    Failed,
}

#[allow(dead_code)]
impl RunStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
        }
    }
}

/// Cloud Run / Process execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Running,
    Success,
    Failed,
    Unknown,
}

impl ExecutionStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => Self::Running,
            "ready" | "completed" | "success" => Self::Success,
            "failed" | "error" => Self::Failed,
            _ => Self::Unknown,
        }
    }

    pub fn to_task_status(&self) -> TaskStatus {
        match self {
            Self::Running => TaskStatus::Running,
            Self::Success => TaskStatus::Success,
            Self::Failed => TaskStatus::Failed,
            Self::Unknown => TaskStatus::Running,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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

#[allow(dead_code)]
impl Run {
    pub fn status(&self) -> RunStatus {
        RunStatus::from_str(&self.status).unwrap_or(RunStatus::Pending)
    }

    pub fn status_str(&self) -> &str {
        &self.status
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_index: i32,
    status: String,
    pub execution_name: Option<String>,
    pub params: Option<Json<serde_json::Value>>,
    pub output: Option<Json<serde_json::Value>>,
    pub error: Option<String>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Task {
    pub fn status(&self) -> TaskStatus {
        TaskStatus::from_str(&self.status).unwrap_or(TaskStatus::Pending)
    }

    pub fn status_str(&self) -> &str {
        &self.status
    }
}

/// Combined task and workflow data to avoid N+1 queries
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TaskWithWorkflow {
    // Task fields
    pub task_id: Uuid,
    pub run_id: Uuid,
    #[allow(dead_code)]
    pub task_index: i32,
    task_status: String,
    pub execution_name: Option<String>,
    pub params: Option<Json<serde_json::Value>>,

    // Workflow fields
    pub workflow_id: Uuid,
    #[allow(dead_code)]
    executor_type: String,
    pub job_name: String,
    #[allow(dead_code)]
    pub project: String,
    #[allow(dead_code)]
    pub region: String,
}

impl TaskWithWorkflow {
    pub fn task_status(&self) -> TaskStatus {
        TaskStatus::from_str(&self.task_status).unwrap_or(TaskStatus::Pending)
    }
}
