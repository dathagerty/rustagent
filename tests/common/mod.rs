use anyhow::Result;
use chrono::Utc;
use rustagent::db::Database;
use rustagent::graph::store::{GraphStore, SqliteGraphStore};
use rustagent::graph::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Helper to create a test goal node
pub fn create_test_goal(id: &str, project_id: &str, title: &str) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        project_id: project_id.to_string(),
        node_type: NodeType::Goal,
        title: title.to_string(),
        description: "Test goal".to_string(),
        status: NodeStatus::Pending,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    }
}

/// Helper to create a test task node (can optionally accept priority)
pub fn create_test_task(id: &str, project_id: &str, title: &str, status: NodeStatus) -> GraphNode {
    create_test_task_with_priority(id, project_id, title, status, Some(Priority::Medium))
}

/// Helper to create a test task node with specific priority
pub fn create_test_task_with_priority(
    id: &str,
    project_id: &str,
    title: &str,
    status: NodeStatus,
    priority: Option<Priority>,
) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        project_id: project_id.to_string(),
        node_type: NodeType::Task,
        title: title.to_string(),
        description: "Test task".to_string(),
        status,
        priority,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    }
}

/// Helper to create a test observation node
pub fn create_test_observation(
    id: &str,
    project_id: &str,
    title: &str,
    description: &str,
) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        project_id: project_id.to_string(),
        node_type: NodeType::Observation,
        title: title.to_string(),
        description: description.to_string(),
        status: NodeStatus::Active,
        priority: None,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    }
}

/// Helper to set up a test database with a project (graph store only)
pub async fn setup_test_env() -> Result<(Database, SqliteGraphStore)> {
    let db = Database::open_in_memory().await?;
    let graph_store = SqliteGraphStore::new(db.clone());

    // Create a test project by directly inserting into the database
    let db_for_project = db.clone();
    db_for_project
        .connection()
        .call(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO projects (id, name, path, registered_at, config_overrides, metadata)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "proj-1",
                    "proj-1",
                    "/tmp/proj-1",
                    &now,
                    None::<String>,
                    "{}"
                ],
            )?;
            Ok(())
        })
        .await?;

    Ok((db, graph_store))
}

/// Helper to set up a test database with a project (includes project store)
pub async fn setup_test_env_with_project()
-> Result<(Database, SqliteGraphStore, rustagent::project::ProjectStore)> {
    let db = Database::open_in_memory().await?;
    let proj_store = rustagent::project::ProjectStore::new(db.clone());
    let graph_store = SqliteGraphStore::new(db.clone());

    // Create a test project by directly inserting into the database
    let db_for_project = db.clone();
    db_for_project
        .connection()
        .call(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO projects (id, name, path, registered_at, config_overrides, metadata)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "proj-1",
                    "proj-1",
                    "/tmp/proj-1",
                    &now,
                    None::<String>,
                    "{}"
                ],
            )?;
            Ok(())
        })
        .await?;

    Ok((db, graph_store, proj_store))
}

/// Helper to set up a test database with a project (wrapped in Arc for concurrency tests)
pub async fn setup_test_env_concurrent() -> Result<(Database, Arc<SqliteGraphStore>)> {
    let db = Database::open_in_memory().await?;
    let graph_store = Arc::new(SqliteGraphStore::new(db.clone()));

    // Create a test project by directly inserting into the database
    let db_for_project = db.clone();
    db_for_project
        .connection()
        .call(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO projects (id, name, path, registered_at, config_overrides, metadata)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "proj-1",
                    "proj-1",
                    "/tmp/proj-1",
                    &now,
                    None::<String>,
                    "{}"
                ],
            )?;
            Ok(())
        })
        .await?;

    Ok((db, graph_store))
}
