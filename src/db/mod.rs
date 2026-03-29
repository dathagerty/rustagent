use anyhow::Result;
use std::path::Path;
use tokio_rusqlite::Connection;

pub mod migrations;

/// Database wrapper providing async access to SQLite
#[derive(Clone)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open a database at the given path
    pub async fn open(path: &Path) -> Result<Self> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open the connection
        let conn = Connection::open(path).await?;

        // Initialize pragmas
        Self::init_pragmas(&conn).await?;

        // Run migrations
        migrations::run_migrations(&conn).await?;

        Ok(Self { conn })
    }

    /// Open an in-memory database (useful for testing)
    pub async fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().await?;

        // Initialize pragmas
        Self::init_pragmas(&conn).await?;

        // Run migrations
        migrations::run_migrations(&conn).await?;

        Ok(Self { conn })
    }

    /// Get a reference to the connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Initialize pragma settings for WAL mode and safety
    async fn init_pragmas(conn: &Connection) -> Result<()> {
        conn.call(|c| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA wal_autocheckpoint = 1000;",
            )?;

            Ok(())
        })
        .await?;

        Ok(())
    }
}
