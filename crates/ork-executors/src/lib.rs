#[cfg(feature = "process")]
pub mod process;

#[cfg(feature = "cloudrun")]
pub mod cloud_run;

pub mod manager;

#[cfg(feature = "process")]
pub use process::ProcessExecutor;

#[cfg(feature = "cloudrun")]
pub use cloud_run::CloudRunClient;

pub use manager::ExecutorManager;

// Re-export the Executor trait from ork-core
pub use ork_core::executor::{Executor, StatusUpdate};
