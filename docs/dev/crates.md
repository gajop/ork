Status: Pending Review

# Crate Structure

```
ork/
├── Cargo.toml              # workspace
├── crates/
│   ├── ork-core/           # types, traits
│   ├── ork-state/          # state store implementations
│   ├── ork-object/         # object store implementations
│   ├── ork-executor/       # executor implementations
│   ├── ork-scheduler/      # scheduler logic
│   ├── ork-worker/         # worker binary
│   └── ork-api/            # HTTP API
└── ork-cli/                # CLI binary
```

## ork-core

Domain types and traits. Minimal dependencies.

```rust
// Workflow definition
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub definition: WorkflowDefinition,
    pub schedule: Option<String>,
    pub version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct WorkflowDefinition {
    pub tasks: Vec<TaskDefinition>,
    pub types: HashMap<String, TypeSchema>,
}

pub struct TaskDefinition {
    pub name: String,
    pub executor: String,
    pub file: Option<String>,
    pub config_file: Option<String>,
    pub input: serde_json::Value,
    pub depends_on: Vec<String>,
    pub timeout_seconds: u32,
    pub retries: u32,
    pub retry_delay_seconds: u32,
    pub parallel: Option<Parallelism>,
    pub concurrency: Option<u32>,
    pub resources: Resources,
    pub env: HashMap<String, EnvValue>,
    pub on_failure: OnFailure,
    pub output_type: Option<String>,
}

pub enum Parallelism {
    Static(u32),
    CountRef(String),    // e.g. "discover.count"
    ForEach(String),     // e.g. "discover.files"
}

pub enum OnFailure {
    Skip,
    Run,
}

// Runtime state
pub struct Run {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub workflow_name: String,
    pub workflow_version: String,
    pub status: RunStatus,
    pub triggered_by: TriggerType,
    pub input: serde_json::Value,
    pub owner: Option<String>,
    pub lease_expires: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub enum RunStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

pub struct TaskRun {
    pub id: Uuid,
    pub run_id: Uuid,
    pub task_name: String,
    pub status: TaskStatus,
    pub attempt: u32,
    pub max_retries: u32,
    pub parallel_index: Option<u32>,
    pub parallel_count: Option<u32>,
    pub executor_id: Option<String>,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub enum TaskStatus {
    Pending,
    Dispatched,
    Running,
    Success,
    Failed,
    Skipped,
}

// Context passed to tasks
pub struct Context {
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub task_name: String,
    pub workflow_name: String,
    pub attempt: u32,
    pub parallel_index: Option<u32>,
    pub parallel_count: Option<u32>,
    pub log: Logger,
    pub metrics: MetricsRecorder,
}
```

## ork-state

State store trait and implementations.

```rust
#[async_trait]
pub trait StateStore: Send + Sync {
    // Workflows
    async fn create_workflow(&self, workflow: &Workflow) -> Result<()>;
    async fn get_workflow(&self, id: Uuid) -> Result<Option<Workflow>>;
    async fn get_workflow_by_name(&self, name: &str) -> Result<Option<Workflow>>;
    async fn list_workflows(&self) -> Result<Vec<Workflow>>;
    async fn delete_workflow(&self, id: Uuid) -> Result<()>;

    // Runs
    async fn create_run(&self, run: &Run, tasks: &[TaskRun]) -> Result<()>;
    async fn get_run(&self, id: Uuid) -> Result<Option<Run>>;
    async fn list_runs(&self, workflow_id: Option<Uuid>, limit: u32) -> Result<Vec<Run>>;
    async fn update_run(&self, run: &Run) -> Result<()>;

    // Task runs
    async fn get_task_runs(&self, run_id: Uuid) -> Result<Vec<TaskRun>>;
    async fn update_task_run(&self, task: &TaskRun) -> Result<()>;
    async fn try_acquire_task(&self, run_id: Uuid) -> Result<Option<TaskRun>>;

    // Schedules
    async fn get_due_schedules(&self) -> Result<Vec<Schedule>>;
    async fn update_schedule(&self, schedule: &Schedule) -> Result<()>;

    // Leases
    async fn acquire_lease(&self, run_id: Uuid, owner: &str) -> Result<bool>;
    async fn renew_lease(&self, run_id: Uuid, owner: &str) -> Result<bool>;
    async fn release_lease(&self, run_id: Uuid, owner: &str) -> Result<()>;
}

pub struct FirestoreState { /* ... */ }
pub struct DynamoDBState { /* ... */ }
pub struct CosmosDBState { /* ... */ }
pub struct PostgresState { /* ... */ }
```

## ork-object

Object store trait and implementations.

```rust
#[async_trait]
pub trait ObjectStore: Send + Sync {
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn delete(&self, path: &str) -> Result<()>;
}

pub struct GcsStore { /* ... */ }
pub struct S3Store { /* ... */ }
pub struct AzureBlobStore { /* ... */ }
pub struct LocalStore { /* ... */ }

// Typed wrappers
impl<S: ObjectStore> TaskSpecStore<S> {
    pub async fn write(&self, run_id: Uuid, task_id: Uuid, spec: &TaskSpec) -> Result<()>;
    pub async fn read(&self, run_id: Uuid, task_id: Uuid) -> Result<Option<TaskSpec>>;
}

impl<S: ObjectStore> TaskStatusStore<S> {
    pub async fn write(&self, run_id: Uuid, task_id: Uuid, status: &TaskStatusFile) -> Result<()>;
    pub async fn read(&self, run_id: Uuid, task_id: Uuid) -> Result<Option<TaskStatusFile>>;
}

impl<S: ObjectStore> TaskOutputStore<S> {
    pub async fn write(&self, run_id: Uuid, task_id: Uuid, output: &serde_json::Value) -> Result<()>;
    pub async fn read(&self, run_id: Uuid, task_id: Uuid) -> Result<Option<serde_json::Value>>;
}
```

## ork-executor

Executor trait and implementations.

```rust
pub struct ExecutionConfig {
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub task_name: String,
    pub workflow_name: String,
    pub executor: String,
    pub file: Option<String>,
    pub config_file: Option<String>,
    pub spec_path: String,
    pub resources: Resources,
    pub timeout_seconds: u32,
    pub env: HashMap<String, String>,
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn dispatch(&self, config: ExecutionConfig) -> Result<String>;  // returns executor_id
    async fn status(&self, executor_id: &str) -> Result<ExecutionStatus>;
    async fn cancel(&self, executor_id: &str) -> Result<()>;
}

pub struct CloudRunExecutor { /* ... */ }
pub struct FargateExecutor { /* ... */ }
pub struct ContainerAppExecutor { /* ... */ }
pub struct DockerExecutor { /* ... */ }
pub struct SubprocessExecutor { /* ... */ }
```

## ork-scheduler

Scheduler logic.

```rust
pub struct Scheduler<S: StateStore, O: ObjectStore, E: Executor> {
    state: S,
    objects: O,
    executor: E,
    config: SchedulerConfig,
    id: String,
}

pub struct SchedulerConfig {
    pub cron_check_interval: Duration,
    pub dispatch_interval: Duration,
    pub monitor_interval: Duration,
    pub heartbeat_timeout: Duration,
    pub lease_duration: Duration,
}

impl<S, O, E> Scheduler<S, O, E> {
    pub async fn run(&self) -> Result<()>;
    
    async fn check_cron(&self) -> Result<()>;
    async fn dispatch_tasks(&self) -> Result<()>;
    async fn monitor_tasks(&self) -> Result<()>;
    async fn renew_leases(&self) -> Result<()>;
}
```

## ork-worker

Worker binary.

```rust
pub struct Worker<O: ObjectStore> {
    objects: O,
    spec: TaskSpec,
    ctx: Context,
}

impl<O: ObjectStore> Worker<O> {
    pub async fn run(&self) -> Result<()> {
        self.write_status(TaskFileStatus::Running).await?;
        
        let heartbeat = self.start_heartbeat();
        let result = self.execute().await;
        heartbeat.stop();
        
        match result {
            Ok(output) => {
                self.write_output(&output).await?;
                self.write_status(TaskFileStatus::Success).await?;
                Ok(())
            }
            Err(e) => {
                self.write_status_error(TaskFileStatus::Failed, &e).await?;
                Err(e)
            }
        }
    }
}
```

## ork-api

HTTP API using axum.

```rust
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/workflows", get(list_workflows).post(create_workflow))
        .route("/workflows/:name", get(get_workflow).delete(delete_workflow))
        .route("/workflows/:name/runs", post(create_run))
        .route("/runs", get(list_runs))
        .route("/runs/:id", get(get_run))
        .route("/runs/:id/cancel", post(cancel_run))
        .route("/runs/:id/retry", post(retry_run))
        .route("/runs/:id/tasks", get(list_task_runs))
        .route("/runs/:id/tasks/:name/logs", get(get_task_logs))
        .with_state(state)
}
```

## ork-cli

CLI using clap.

```rust
#[derive(Parser)]
enum Cli {
    Workflow(WorkflowCmd),
    Run(RunCmd),
    Schema(SchemaCmd),
    Dev,
}

#[derive(Subcommand)]
enum WorkflowCmd {
    Deploy { path: PathBuf },
    List,
    Status { name: String },
    Delete { name: String },
}

#[derive(Subcommand)]
enum RunCmd {
    Start { workflow: String, #[arg(long)] input: Vec<String> },
    Status { id: Uuid },
    List { workflow: Option<String> },
    Logs { id: Uuid, #[arg(long)] task: Option<String>, #[arg(long)] follow: bool },
    Cancel { id: Uuid },
    Retry { id: Uuid, #[arg(long)] task: Option<String> },
}

#[derive(Subcommand)]
enum SchemaCmd {
    Export { file: PathBuf },
}
```
