use rustagent::db::Database;
use tempfile::TempDir;

#[tokio::test]
async fn test_database_open_creates_file() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let db = Database::open(&db_path).await.unwrap();

    // Verify the file was created
    assert!(db_path.exists());
    assert!(db_path.is_file());

    // Connection should be accessible
    let conn = db.connection();
    conn.call(|c| {
        // Verify basic pragma
        let mut stmt = c.prepare("PRAGMA database_list")?;
        let _db_name: String = stmt.query_row([], |row| row.get(1))?;
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_database_pragma_wal_mode() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db = Database::open(&db_path).await.unwrap();

    let conn = db.connection();
    let journal_mode = conn
        .call(|c| {
            let mut stmt = c.prepare("PRAGMA journal_mode")?;
            let mode: String = stmt.query_row([], |row| row.get::<_, String>(0))?;
            Ok(mode)
        })
        .await
        .unwrap();

    assert_eq!(journal_mode.to_lowercase(), "wal");
}

#[tokio::test]
async fn test_database_pragma_foreign_keys() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let foreign_keys = conn
        .call(|c| {
            let mut stmt = c.prepare("PRAGMA foreign_keys")?;
            let value: i32 = stmt.query_row([], |row| row.get::<_, i32>(0))?;
            Ok(value)
        })
        .await
        .unwrap();

    assert_eq!(foreign_keys, 1);
}

#[tokio::test]
async fn test_database_schema_tables_exist() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let tables = conn
        .call(|c| {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
            )?;
            let tables = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<String>, _>>()?;
            Ok(tables)
        })
        .await
        .unwrap();

    let expected_tables = vec![
        "edges",
        "nodes",
        "nodes_fts",
        "projects",
        "schema_version",
        "sessions",
        "worker_conversations",
    ];

    for expected in expected_tables {
        assert!(
            tables.contains(&expected.to_string()),
            "Missing table: {}",
            expected
        );
    }
}

#[tokio::test]
async fn test_schema_version_after_fresh_init() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let version = conn
        .call(|c| {
            let mut stmt = c.prepare("SELECT version FROM schema_version LIMIT 1")?;
            let v: u32 = stmt.query_row([], |row| row.get::<_, u32>(0))?;
            Ok(v)
        })
        .await
        .unwrap();

    assert_eq!(version, 1);
}

#[tokio::test]
async fn test_projects_table_has_correct_schema() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let columns = conn
        .call(|c| {
            let mut stmt = c.prepare("PRAGMA table_info(projects)")?;
            let cols = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(cols)
        })
        .await
        .unwrap();

    let expected_cols = vec!["id", "name", "path", "registered_at", "config_overrides", "metadata"];

    for (col_name, _) in columns {
        assert!(
            expected_cols.contains(&col_name.as_str()),
            "Unexpected column: {}",
            col_name
        );
    }
}

#[tokio::test]
async fn test_nodes_table_has_correct_schema() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let columns = conn
        .call(|c| {
            let mut stmt = c.prepare("PRAGMA table_info(nodes)")?;
            let cols = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(cols)
        })
        .await
        .unwrap();

    let expected_cols = vec![
        "id",
        "project_id",
        "node_type",
        "title",
        "description",
        "status",
        "priority",
        "assigned_to",
        "created_by",
        "labels",
        "created_at",
        "started_at",
        "completed_at",
        "blocked_reason",
        "metadata",
    ];

    for expected in expected_cols {
        assert!(
            columns.contains(&expected.to_string()),
            "Missing column in nodes table: {}",
            expected
        );
    }
}

#[tokio::test]
async fn test_fts_table_exists() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let exists = conn
        .call(|c| {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='nodes_fts'",
            )?;
            let result = stmt.exists([])?;
            Ok(result)
        })
        .await
        .unwrap();

    assert!(exists, "nodes_fts virtual table does not exist");
}

#[tokio::test]
async fn test_triggers_exist() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let triggers = conn
        .call(|c| {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='trigger' ORDER BY name",
            )?;
            let triggers = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(triggers)
        })
        .await
        .unwrap();

    let expected_triggers = vec!["nodes_ai", "nodes_ad", "nodes_au"];

    for expected in expected_triggers {
        assert!(
            triggers.contains(&expected.to_string()),
            "Missing trigger: {}",
            expected
        );
    }
}

#[tokio::test]
async fn test_opening_existing_database_does_not_error() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create database first time
    let _db1 = Database::open(&db_path).await.unwrap();
    drop(_db1);

    // Open again - should work
    let _db2 = Database::open(&db_path).await.unwrap();
}

#[tokio::test]
async fn test_database_clone() {
    let db = Database::open_in_memory().await.unwrap();

    let db_clone = db.clone();

    // Both should be able to access the database
    let conn1 = db.connection();
    let conn2 = db_clone.connection();

    let result1 = conn1
        .call(|c| {
            let mut stmt = c.prepare("SELECT version FROM schema_version")?;
            let version: u32 = stmt.query_row([], |row| row.get::<_, u32>(0))?;
            Ok(version)
        })
        .await
        .unwrap();

    let result2 = conn2
        .call(|c| {
            let mut stmt = c.prepare("SELECT version FROM schema_version")?;
            let version: u32 = stmt.query_row([], |row| row.get::<_, u32>(0))?;
            Ok(version)
        })
        .await
        .unwrap();

    assert_eq!(result1, result2);
}

#[tokio::test]
async fn test_indexes_exist() {
    let db = Database::open_in_memory().await.unwrap();

    let conn = db.connection();
    let indexes = conn
        .call(|c| {
            let mut stmt = c.prepare(
                "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name",
            )?;
            let indexes = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(indexes)
        })
        .await
        .unwrap();

    let expected_indexes = vec![
        "idx_edges_from",
        "idx_edges_to",
        "idx_edges_type",
        "idx_nodes_project",
        "idx_nodes_status",
        "idx_nodes_type",
    ];

    for expected in expected_indexes {
        assert!(
            indexes.contains(&expected.to_string()),
            "Missing index: {}",
            expected
        );
    }
}
