#[cfg(feature = "process")]
pub mod process;

#[cfg(feature = "cloudrun")]
pub mod cloud_run;

#[cfg(feature = "library")]
pub mod library;

pub mod manager;
pub mod python_runtime;

#[cfg(feature = "process")]
pub use process::ProcessExecutor;

#[cfg(feature = "cloudrun")]
pub use cloud_run::CloudRunClient;

#[cfg(feature = "library")]
pub use library::LibraryExecutor;

pub use manager::ExecutorManager;

// Re-export the Executor trait from ork-core
pub use ork_core::executor::{Executor, StatusUpdate};
