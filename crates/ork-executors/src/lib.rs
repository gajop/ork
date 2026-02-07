#[cfg(feature = "process")]
pub mod process;

#[cfg(feature = "cloudrun")]
pub mod cloud_run;

#[cfg(feature = "library")]
pub mod library;

#[cfg(feature = "worker")]
pub mod worker;

pub mod manager;
pub mod python_runtime;

#[cfg(feature = "process")]
pub use process::ProcessExecutor;

#[cfg(feature = "cloudrun")]
pub use cloud_run::CloudRunClient;

#[cfg(feature = "library")]
pub use library::LibraryExecutor;

#[cfg(feature = "worker")]
pub use worker::WorkerExecutor;

pub use manager::ExecutorManager;

// Re-export the Executor trait from ork-core
pub use ork_core::executor::{Executor, StatusUpdate};
