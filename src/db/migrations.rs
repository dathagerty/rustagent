use anyhow::Result;
use tokio_rusqlite::Connection;

const CURRENT_VERSION: u32 = 1;

/// Run migrations to set up or upgrade the database schema
pub async fn run_migrations(conn: &Connection) -> Result<()> {
    conn.call(|c| check_and_migrate(c).map_err(tokio_rusqlite::Error::Rusqlite))
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}

/// Check schema version and apply migrations if needed (exposed for testing)
pub fn check_and_migrate(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    // Check if schema_version table exists
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_version'")?;

    let exists = stmt.exists([])?;

    if !exists {
        // Fresh database - create full schema
        create_schema_v1(conn)?;
        return Ok(());
    }

    // Read current version
    let mut stmt = conn.prepare("SELECT version FROM schema_version LIMIT 1")?;
    let db_version: u32 = stmt.query_row([], |row| row.get(0))?;

    if db_version == CURRENT_VERSION {
        // Already at current version
        return Ok(());
    }

    if db_version > CURRENT_VERSION {
        // Database is newer than this binary
        // Return an error to signal this condition
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some(
                "your database was created by a newer version of rustagent, please upgrade"
                    .to_string(),
            ),
        ));
    }

    // Handle future migrations if needed (e.g., db_version < CURRENT_VERSION)
    // For now, this is a fresh v1 implementation
    Ok(())
}

/// Create the initial database schema (version 1)
fn create_schema_v1(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    // Wrap schema creation in a transaction for atomicity using BEGIN/COMMIT
    conn.execute_batch("BEGIN IMMEDIATE")?;

    // Schema version table
    conn.execute(
        "CREATE TABLE schema_version (
            version INTEGER NOT NULL,
            migrated_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "INSERT INTO schema_version (version, migrated_at) VALUES (?, ?)",
        ["1", &chrono::Utc::now().to_rfc3339()],
    )?;

    // Projects table
    conn.execute(
        "CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL,
            registered_at TEXT NOT NULL,
            config_overrides TEXT,
            metadata TEXT NOT NULL DEFAULT '{}'
        )",
        [],
    )?;

    // Nodes table (unified work graph)
    conn.execute(
        "CREATE TABLE nodes (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            node_type TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            priority TEXT,
            assigned_to TEXT,
            created_by TEXT,
            labels TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            started_at TEXT,
            completed_at TEXT,
            blocked_reason TEXT,
            metadata TEXT NOT NULL DEFAULT '{}'
        )",
        [],
    )?;

    conn.execute("CREATE INDEX idx_nodes_project ON nodes(project_id)", [])?;
    conn.execute("CREATE INDEX idx_nodes_type ON nodes(node_type)", [])?;
    conn.execute("CREATE INDEX idx_nodes_status ON nodes(status)", [])?;

    // Edges table (unified relationships)
    conn.execute(
        "CREATE TABLE edges (
            id TEXT PRIMARY KEY,
            edge_type TEXT NOT NULL,
            from_node TEXT NOT NULL REFERENCES nodes(id),
            to_node TEXT NOT NULL REFERENCES nodes(id),
            label TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute("CREATE INDEX idx_edges_from ON edges(from_node)", [])?;
    conn.execute("CREATE INDEX idx_edges_to ON edges(to_node)", [])?;
    conn.execute("CREATE INDEX idx_edges_type ON edges(edge_type)", [])?;

    // Sessions table (temporal)
    conn.execute(
        "CREATE TABLE sessions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id),
            goal_id TEXT NOT NULL REFERENCES nodes(id),
            started_at TEXT NOT NULL,
            ended_at TEXT,
            handoff_notes TEXT,
            agent_ids TEXT NOT NULL DEFAULT '[]',
            summary TEXT
        )",
        [],
    )?;

    // Full-text search virtual table
    conn.execute(
        "CREATE VIRTUAL TABLE nodes_fts USING fts5(
            title,
            description,
            content='nodes',
            content_rowid='rowid'
        )",
        [],
    )?;

    // FTS sync triggers
    conn.execute(
        "CREATE TRIGGER nodes_ai AFTER INSERT ON nodes BEGIN
            INSERT INTO nodes_fts(rowid, title, description)
            VALUES (new.rowid, new.title, new.description);
        END",
        [],
    )?;

    conn.execute(
        "CREATE TRIGGER nodes_ad AFTER DELETE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title, description)
            VALUES ('delete', old.rowid, old.title, old.description);
        END",
        [],
    )?;

    conn.execute(
        "CREATE TRIGGER nodes_au AFTER UPDATE ON nodes BEGIN
            INSERT INTO nodes_fts(nodes_fts, rowid, title, description)
            VALUES ('delete', old.rowid, old.title, old.description);
            INSERT INTO nodes_fts(rowid, title, description)
            VALUES (new.rowid, new.title, new.description);
        END",
        [],
    )?;

    // Worker conversations table
    conn.execute(
        "CREATE TABLE worker_conversations (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id),
            agent_id TEXT NOT NULL,
            task_ids TEXT NOT NULL DEFAULT '[]',
            messages TEXT NOT NULL,
            total_input_tokens INTEGER NOT NULL DEFAULT 0,
            total_output_tokens INTEGER NOT NULL DEFAULT 0,
            started_at TEXT NOT NULL,
            completed_at TEXT
        )",
        [],
    )?;

    conn.execute_batch("COMMIT")?;
    Ok(())
}
