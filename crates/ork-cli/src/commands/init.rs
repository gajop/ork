use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use tracing::info;

use ork_core::database::Database;

#[derive(Args)]
pub struct Init;

impl Init {
    pub async fn execute(self, db: Arc<dyn Database>) -> Result<()> {
        info!("Running database migrations...");
        db.run_migrations().await?;
        println!("✓ Database initialized successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ork_state::SqliteDatabase;

    #[tokio::test]
    async fn test_init_command_runs_migrations() {
        let db = Arc::new(SqliteDatabase::new(":memory:").await.expect("create db"));
        let cmd = Init;
        cmd.execute(db)
            .await
            .expect("init command should run migrations");
    }
}
