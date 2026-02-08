//! Google Cloud Platform job trackers
//!
//! This module contains job tracker implementations for GCP services:
//! - BigQuery
//! - Cloud Run
//! - Dataproc
//!
//! These trackers are only available when the `gcp` feature is enabled.

mod bigquery;
mod cloudrun;
mod dataproc;

pub use bigquery::BigQueryTracker;
#[cfg(test)]
pub(crate) use bigquery::map_bigquery_status;
pub use cloudrun::CloudRunTracker;
pub use dataproc::DataprocTracker;
