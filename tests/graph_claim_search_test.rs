use anyhow::Result;
use chrono::Utc;
use rustagent::db::Database;
use rustagent::graph::store::{GraphStore, SqliteGraphStore};
use rustagent::graph::*;
use std::collections::HashMap;

/// Helper to create a test goal node
fn create_test_goal(id: &str, project_id: &str, title: &str) -> GraphNode {
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

/// Helper to create a test task node
fn create_test_task(id: &str, project_id: &str, title: &str, status: NodeStatus) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        project_id: project_id.to_string(),
        node_type: NodeType::Task,
        title: title.to_string(),
        description: "Test task".to_string(),
        status,
        priority: Some(Priority::Medium),
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
fn create_test_observation(
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

/// Helper to set up a test database with a project
async fn setup_test_env() -> Result<(Database, SqliteGraphStore)> {
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

/// P1b.AC5.1: claim_task atomically sets status Ready->Claimed and assigned_to
#[tokio::test]
async fn test_claim_task_success() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a Ready task
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    let task = create_test_task("ra-a1b2.1", "proj-1", "Claimable Task", NodeStatus::Ready);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Claim the task
    let claimed = store.claim_task("ra-a1b2.1", "agent-1").await?;
    assert!(claimed, "Task should have been claimed successfully");

    // Verify the node was updated
    let claimed_node = store.get_node("ra-a1b2.1").await?;
    assert!(claimed_node.is_some());
    let node = claimed_node.unwrap();
    assert_eq!(node.status, NodeStatus::Claimed);
    assert_eq!(node.assigned_to, Some("agent-1".to_string()));
    assert!(node.started_at.is_some(), "started_at should be set");

    Ok(())
}

/// P1b.AC5.2: If task is not Ready, claim returns false (another worker got it first)
#[tokio::test]
async fn test_claim_task_already_claimed() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a Ready task
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    let task = create_test_task("ra-a1b2.1", "proj-1", "Claimable Task", NodeStatus::Ready);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Claim the task once (should succeed)
    let first_claim = store.claim_task("ra-a1b2.1", "agent-1").await?;
    assert!(first_claim, "First claim should succeed");

    // Try to claim the same task again (should fail because it's no longer Ready)
    let second_claim = store.claim_task("ra-a1b2.1", "agent-2").await?;
    assert!(!second_claim, "Second claim should fail");

    // Verify the task is still assigned to the first agent
    let node = store.get_node("ra-a1b2.1").await?.unwrap();
    assert_eq!(node.assigned_to, Some("agent-1".to_string()));

    Ok(())
}

/// P1b.AC5.2 variant: Claiming a Pending task should fail
#[tokio::test]
async fn test_claim_task_not_ready() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a Pending task (not Ready)
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    let task = create_test_task("ra-a1b2.1", "proj-1", "Pending Task", NodeStatus::Pending);

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Try to claim the task (should fail)
    let claimed = store.claim_task("ra-a1b2.1", "agent-1").await?;
    assert!(!claimed, "Cannot claim a Pending task");

    // Verify the node is still Pending
    let node = store.get_node("ra-a1b2.1").await?.unwrap();
    assert_eq!(node.status, NodeStatus::Pending);
    assert_eq!(node.assigned_to, None);

    Ok(())
}

/// P1b.AC6.1: search_nodes returns nodes matching title or description via FTS5
#[tokio::test]
async fn test_search_nodes_by_title_and_description() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal and several nodes with different content
    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    store.create_node(&goal).await?;

    // Create nodes with searchable content
    let node1 = create_test_observation(
        "ra-a1b2.1",
        "proj-1",
        "Authentication Bug",
        "Found a critical authentication issue",
    );
    let node2 = create_test_observation(
        "ra-a1b2.2",
        "proj-1",
        "Database Query",
        "Query performance issue with authentication",
    );
    let node3 = create_test_observation("ra-a1b2.3", "proj-1", "UI Bug", "Button styling issue");

    store.create_node(&node1).await?;
    store.create_node(&node2).await?;
    store.create_node(&node3).await?;

    // Search for "authentication"
    let results = store.search_nodes("authentication", None, None, 10).await?;

    // Should find nodes 1 and 2 (both have "authentication" in title or description)
    assert!(
        results.len() >= 2,
        "Should find at least 2 nodes with 'authentication'"
    );
    let ids: Vec<String> = results.iter().map(|n| n.id.clone()).collect();
    assert!(ids.contains(&"ra-a1b2.1".to_string()));
    assert!(ids.contains(&"ra-a1b2.2".to_string()));
    assert!(!ids.contains(&"ra-a1b2.3".to_string()));

    Ok(())
}

/// P1b.AC6.2: Search can filter by project_id
#[tokio::test]
async fn test_search_nodes_filter_by_project() -> Result<()> {
    let (db, store) = setup_test_env().await?;

    // Create a second project
    let db_proj = db.clone();
    db_proj
        .connection()
        .call(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO projects (id, name, path, registered_at, config_overrides, metadata)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "proj-2",
                    "proj-2",
                    "/tmp/proj-2",
                    &now,
                    None::<String>,
                    "{}"
                ],
            )?;
            Ok(())
        })
        .await?;

    // Create nodes in different projects
    let goal1 = create_test_goal("ra-a1b2", "proj-1", "Goal 1");
    let goal2 = create_test_goal("ra-c3d4", "proj-2", "Goal 2");

    let node1 = create_test_observation(
        "ra-a1b2.1",
        "proj-1",
        "Authentication in Project 1",
        "Details",
    );
    let node2 = create_test_observation(
        "ra-c3d4.1",
        "proj-2",
        "Authentication in Project 2",
        "Details",
    );

    store.create_node(&goal1).await?;
    store.create_node(&goal2).await?;
    store.create_node(&node1).await?;
    store.create_node(&node2).await?;

    // Search for "authentication" in proj-1 only
    let results = store
        .search_nodes("authentication", Some("proj-1"), None, 10)
        .await?;

    // Should only find nodes from proj-1
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "ra-a1b2.1");
    assert_eq!(results[0].project_id, "proj-1");

    Ok(())
}

/// P1b.AC6.2: Search can filter by node_type
#[tokio::test]
async fn test_search_nodes_filter_by_type() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal
    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal with authentication");
    store.create_node(&goal).await?;

    // Create a task and an observation, both with "authentication" in the content
    let task = create_test_task(
        "ra-a1b2.1",
        "proj-1",
        "Fix authentication",
        NodeStatus::Pending,
    );
    let observation = create_test_observation(
        "ra-a1b2.2",
        "proj-1",
        "Authentication Overview",
        "System details",
    );

    store.create_node(&task).await?;
    store.create_node(&observation).await?;

    // Search for "authentication" filtered by Task type
    let task_results = store
        .search_nodes("authentication", None, Some(NodeType::Task), 10)
        .await?;

    // Should find the task
    assert!(!task_results.is_empty());
    assert!(task_results.iter().any(|n| n.id == "ra-a1b2.1"));

    // Search for "authentication" filtered by Observation type
    let observation_results = store
        .search_nodes("authentication", None, Some(NodeType::Observation), 10)
        .await?;

    // Should find the observation
    assert!(!observation_results.is_empty());
    assert!(observation_results.iter().any(|n| n.id == "ra-a1b2.2"));

    // Search for "authentication" filtered by Goal type
    let goal_results = store
        .search_nodes("authentication", None, Some(NodeType::Goal), 10)
        .await?;

    // Should find the goal
    assert!(!goal_results.is_empty());
    assert!(goal_results.iter().any(|n| n.id == "ra-a1b2"));

    Ok(())
}

/// P1b.AC6.1: Search respects limit parameter
#[tokio::test]
async fn test_search_nodes_respects_limit() -> Result<()> {
    let (_db, store) = setup_test_env().await?;

    // Create a goal
    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    store.create_node(&goal).await?;

    // Create 5 nodes with "test" in their content
    for i in 1..=5 {
        let node = create_test_observation(
            &format!("ra-a1b2.{}", i),
            "proj-1",
            &format!("Test Node {}", i),
            "This is a test",
        );
        store.create_node(&node).await?;
    }

    // Search with limit 3
    let results = store.search_nodes("test", None, None, 3).await?;

    // Should return at most 3 results
    assert!(results.len() <= 3, "Results should respect limit of 3");

    Ok(())
}
