mod common;

use anyhow::Result;
use chrono::Utc;
use common::{create_test_goal, create_test_task, setup_test_env_with_project};
use rustagent::graph::store::GraphStore;
use rustagent::graph::*;
use std::collections::HashMap;

#[tokio::test]
async fn test_create_and_get_goal_node() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Test Goal");
    store.create_node(&goal).await?;

    let retrieved = store.get_node("ra-a1b2").await?;
    assert!(retrieved.is_some());

    let node = retrieved.unwrap();
    assert_eq!(node.id, "ra-a1b2");
    assert_eq!(node.title, "Test Goal");
    assert_eq!(node.project_id, "proj-1");

    Ok(())
}

#[tokio::test]
async fn test_get_nonexistent_node() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let retrieved = store.get_node("nonexistent-id").await?;
    assert!(retrieved.is_none());

    Ok(())
}

#[tokio::test]
async fn test_update_node_status() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    store.create_node(&goal).await?;

    let task = create_test_task("ra-a1b2.1", "proj-1", "Test Task", NodeStatus::Pending);
    store.create_node(&task).await?;

    // Update status to InProgress (valid for Task)
    store
        .update_node("ra-a1b2.1", Some(NodeStatus::InProgress), None, None, None)
        .await?;

    let updated = store.get_node("ra-a1b2.1").await?;
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().status, NodeStatus::InProgress);

    Ok(())
}

#[tokio::test]
async fn test_update_node_title() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    store.create_node(&goal).await?;

    let task = create_test_task("ra-a1b2.1", "proj-1", "Original Title", NodeStatus::Pending);
    store.create_node(&task).await?;

    // Update title
    store
        .update_node("ra-a1b2.1", None, Some("New Title"), None, None)
        .await?;

    let updated = store.get_node("ra-a1b2.1").await?;
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().title, "New Title");

    Ok(())
}

#[tokio::test]
async fn test_add_and_get_edge() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    let task = create_test_task("ra-a1b2.1", "proj-1", "Task", NodeStatus::Pending);
    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    // Add an edge (this should already be created via the parent relationship)
    let edge = GraphEdge {
        id: generate_edge_id(),
        edge_type: EdgeType::DependsOn,
        from_node: "ra-a1b2.1".to_string(),
        to_node: "ra-a1b2".to_string(),
        label: Some("depends".to_string()),
        created_at: Utc::now(),
    };
    store.add_edge(&edge).await?;

    // Get edges
    let edges = store
        .get_edges("ra-a1b2", rustagent::graph::store::EdgeDirection::Incoming)
        .await?;
    assert!(!edges.is_empty());
    assert_eq!(edges[0].0.edge_type, EdgeType::DependsOn);

    Ok(())
}

#[tokio::test]
async fn test_get_children() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    store.create_node(&goal).await?;

    // Create children (via parent relationship in create_node)
    let task1 = create_test_task("ra-a1b2.1", "proj-1", "Task 1", NodeStatus::Pending);
    let task2 = create_test_task("ra-a1b2.2", "proj-1", "Task 2", NodeStatus::Pending);
    store.create_node(&task1).await?;
    store.create_node(&task2).await?;

    let children = store.get_children("ra-a1b2").await?;
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].0.id, "ra-a1b2.1");
    assert_eq!(children[1].0.id, "ra-a1b2.2");

    Ok(())
}

#[tokio::test]
async fn test_get_subtree() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-a1b2", "proj-1", "Goal");
    store.create_node(&goal).await?;

    let task = create_test_task("ra-a1b2.1", "proj-1", "Task", NodeStatus::Pending);
    store.create_node(&task).await?;

    // Create a subtask
    let subtask = GraphNode {
        id: "ra-a1b2.1.1".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Task,
        title: "Subtask".to_string(),
        description: "A subtask".to_string(),
        status: NodeStatus::Pending,
        priority: Some(Priority::Low),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    store.create_node(&subtask).await?;

    let subtree = store.get_subtree("ra-a1b2").await?;
    assert_eq!(subtree.len(), 3); // goal + task + subtask
    assert!(subtree.iter().any(|n| n.id == "ra-a1b2"));
    assert!(subtree.iter().any(|n| n.id == "ra-a1b2.1"));
    assert!(subtree.iter().any(|n| n.id == "ra-a1b2.1.1"));

    Ok(())
}

#[tokio::test]
async fn test_get_active_decisions() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let active_decision = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Active Decision".to_string(),
        description: "An active decision".to_string(),
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
    };

    let decided_decision = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Decided Decision".to_string(),
        description: "A decided decision".to_string(),
        status: NodeStatus::Decided,
        priority: None,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };

    let superseded_decision = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Superseded Decision".to_string(),
        description: "A superseded decision".to_string(),
        status: NodeStatus::Superseded,
        priority: None,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };

    store.create_node(&active_decision).await?;
    store.create_node(&decided_decision).await?;
    store.create_node(&superseded_decision).await?;

    let active = store.get_active_decisions("proj-1").await?;
    assert_eq!(active.len(), 2); // Active + Decided, but not Superseded
    assert!(active.iter().any(|n| n.status == NodeStatus::Active));
    assert!(active.iter().any(|n| n.status == NodeStatus::Decided));
    assert!(!active.iter().any(|n| n.status == NodeStatus::Superseded));

    Ok(())
}

#[tokio::test]
async fn test_get_full_graph() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-goal1", "proj-1", "Goal");
    store.create_node(&goal).await?;

    let task1 = create_test_task("ra-goal1.1", "proj-1", "Task 1", NodeStatus::Pending);
    let task2 = create_test_task("ra-goal1.2", "proj-1", "Task 2", NodeStatus::Pending);
    store.create_node(&task1).await?;
    store.create_node(&task2).await?;

    // Add an edge between tasks
    let edge = GraphEdge {
        id: generate_edge_id(),
        edge_type: EdgeType::DependsOn,
        from_node: "ra-goal1.2".to_string(),
        to_node: "ra-goal1.1".to_string(),
        label: None,
        created_at: Utc::now(),
    };
    store.add_edge(&edge).await?;

    let graph = store.get_full_graph("ra-goal1").await?;
    assert_eq!(graph.nodes.len(), 3); // goal + 2 tasks
    assert!(!graph.edges.is_empty()); // Contains edges + DependsOn edge

    Ok(())
}

#[tokio::test]
async fn test_search_nodes_by_title() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Goal,
        title: "Authentication System".to_string(),
        description: "Build a secure auth system".to_string(),
        status: NodeStatus::Active,
        priority: Some(Priority::Critical),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    store.create_node(&goal).await?;

    // FTS5 might need a moment to sync, but our test should work
    let results = store
        .search_nodes("authentication", Some("proj-1"), None, 10)
        .await?;
    assert!(!results.is_empty());
    assert!(results.iter().any(|n| n.title.contains("Authentication")));

    Ok(())
}

#[tokio::test]
async fn test_search_nodes_with_type_filter() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Goal,
        title: "Test Goal".to_string(),
        description: "Test description".to_string(),
        status: NodeStatus::Active,
        priority: Some(Priority::Medium),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };

    let task = GraphNode {
        id: generate_goal_id(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Task,
        title: "Test Task".to_string(),
        description: "Test description".to_string(),
        status: NodeStatus::Pending,
        priority: Some(Priority::Low),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };

    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    let goal_results = store
        .search_nodes("test", Some("proj-1"), Some(NodeType::Goal), 10)
        .await?;
    assert!(!goal_results.is_empty());
    assert!(goal_results.iter().all(|n| n.node_type == NodeType::Goal));

    let task_results = store
        .search_nodes("test", Some("proj-1"), Some(NodeType::Task), 10)
        .await?;
    assert!(!task_results.is_empty());
    assert!(task_results.iter().all(|n| n.node_type == NodeType::Task));

    Ok(())
}

#[tokio::test]
async fn test_claim_task() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let task = create_test_task("ra-task1", "proj-1", "Test Task", NodeStatus::Ready);
    store.create_node(&task).await?;

    // Claim the task
    let claimed = store.claim_task("ra-task1", "agent-1").await?;
    assert!(claimed);

    // Verify status changed
    let updated = store.get_node("ra-task1").await?.unwrap();
    assert_eq!(updated.status, NodeStatus::Claimed);
    assert_eq!(updated.assigned_to, Some("agent-1".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_claim_task_already_claimed() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let task = create_test_task("ra-task1", "proj-1", "Test Task", NodeStatus::Ready);
    store.create_node(&task).await?;

    // Claim it once
    let first = store.claim_task("ra-task1", "agent-1").await?;
    assert!(first);

    // Try to claim it again
    let second = store.claim_task("ra-task1", "agent-2").await?;
    assert!(!second); // Should fail

    Ok(())
}

#[tokio::test]
async fn test_next_child_seq() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let parent = create_test_goal("ra-parent", "proj-1", "Parent");
    store.create_node(&parent).await?;

    // Get next sequence
    let seq1 = store.next_child_seq("ra-parent").await?;
    assert_eq!(seq1, 1);

    // Get next sequence again
    let seq2 = store.next_child_seq("ra-parent").await?;
    assert_eq!(seq2, 2);

    // Get next sequence again
    let seq3 = store.next_child_seq("ra-parent").await?;
    assert_eq!(seq3, 3);

    Ok(())
}

#[tokio::test]
async fn test_query_nodes_by_type() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let goal = create_test_goal("ra-g1", "proj-1", "Goal");
    let task = create_test_task("ra-t1", "proj-1", "Task", NodeStatus::Pending);
    store.create_node(&goal).await?;
    store.create_node(&task).await?;

    let query = rustagent::graph::store::NodeQuery {
        node_type: Some(NodeType::Goal),
        status: None,
        project_id: Some("proj-1".to_string()),
        parent_id: None,
        query: None,
    };

    let results = store.query_nodes(&query).await?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_type, NodeType::Goal);

    Ok(())
}

#[tokio::test]
async fn test_query_nodes_by_status() -> Result<()> {
    let (_db, store, _proj_store) = setup_test_env_with_project().await?;

    let pending_task = create_test_task("ra-t1", "proj-1", "Pending Task", NodeStatus::Pending);
    let active_task = create_test_task("ra-t2", "proj-1", "Active Task", NodeStatus::Active);
    store.create_node(&pending_task).await?;
    store.create_node(&active_task).await?;

    let query = rustagent::graph::store::NodeQuery {
        node_type: Some(NodeType::Task),
        status: Some(NodeStatus::Pending),
        project_id: Some("proj-1".to_string()),
        parent_id: None,
        query: None,
    };

    let results = store.query_nodes(&query).await?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "ra-t1");

    Ok(())
}
