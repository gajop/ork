mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod workflows;

pub use core::PostgresDatabase;

use crate::delegation::impl_database_delegates;

impl_database_delegates!(PostgresDatabase, PostgresDatabase::run_migrations);
