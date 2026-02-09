mod core;
mod deferred_jobs;
mod runs;
mod tasks;
mod workflows;

pub use core::SqliteDatabase;

use crate::delegation::impl_database_delegates;

impl_database_delegates!(SqliteDatabase, SqliteDatabase::run_migrations);
