use anyhow::Result;
use chrono::Utc;
use rustagent::graph::session::SessionStore;
use rustagent::graph::store::GraphStore;
use rustagent::graph::*;
use std::collections::HashMap;

mod common;
use common::{create_test_goal, create_test_task};

#[tokio::test]
async fn test_ac1_1_create_session_returns_valid_session() -> Result<()> {
    // P1c.AC1.1: create_session(goal_id) creates a session record with start time and goal reference
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal first
    let goal = create_test_goal("ra-1234", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    let session = session_store.create_session("proj-1", "ra-1234").await?;

    assert!(session.id.starts_with("sess-"));
    assert_eq!(session.project_id, "proj-1");
    assert_eq!(session.goal_id, "ra-1234");
    assert!(session.started_at <= Utc::now());
    assert!(session.ended_at.is_none());
    assert!(session.handoff_notes.is_none());
    assert_eq!(session.agent_ids, vec![] as Vec<String>);

    Ok(())
}

#[tokio::test]
async fn test_ac1_2_end_session_generates_handoff_notes() -> Result<()> {
    // P1c.AC1.2: end_session(session_id) generates deterministic handoff notes from graph state
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal and some tasks
    let goal = create_test_goal("ra-1234", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create some tasks with different statuses
    let task1 = create_test_task("ra-1234.1", "proj-1", "Task 1", NodeStatus::Completed);
    let task2 = create_test_task("ra-1234.2", "proj-1", "Task 2", NodeStatus::Ready);
    let task3 = create_test_task("ra-1234.3", "proj-1", "Task 3", NodeStatus::Blocked);

    graph_store.create_node(&task1).await?;
    graph_store.create_node(&task2).await?;
    graph_store.create_node(&task3).await?;

    // Add edges to make them part of the goal
    graph_store
        .add_edge(&GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::Contains,
            from_node: "ra-1234".to_string(),
            to_node: "ra-1234.1".to_string(),
            label: None,
            created_at: Utc::now(),
        })
        .await?;

    graph_store
        .add_edge(&GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::Contains,
            from_node: "ra-1234".to_string(),
            to_node: "ra-1234.2".to_string(),
            label: None,
            created_at: Utc::now(),
        })
        .await?;

    graph_store
        .add_edge(&GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::Contains,
            from_node: "ra-1234".to_string(),
            to_node: "ra-1234.3".to_string(),
            label: None,
            created_at: Utc::now(),
        })
        .await?;

    // Create a session
    let session = session_store.create_session("proj-1", "ra-1234").await?;

    // End the session
    session_store.end_session(&session.id, &graph_store).await?;

    // Verify the session now has handoff notes
    let ended_session = session_store
        .get_session(&session.id)
        .await?
        .expect("session should exist");

    assert!(ended_session.ended_at.is_some());
    assert!(ended_session.handoff_notes.is_some());

    Ok(())
}

#[tokio::test]
async fn test_ac1_3_handoff_notes_contain_all_sections() -> Result<()> {
    // P1c.AC1.3: Handoff notes contain Done, Remaining, Blocked, and Decisions Made sections
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal
    let goal = create_test_goal("ra-5678", "proj-1", "Complex Goal");
    graph_store.create_node(&goal).await?;

    // Create tasks with different statuses
    let completed_task = create_test_task(
        "ra-5678.1",
        "proj-1",
        "Completed Task",
        NodeStatus::Completed,
    );
    let pending_task = create_test_task("ra-5678.2", "proj-1", "Pending Task", NodeStatus::Pending);
    let blocked_task = create_test_task("ra-5678.3", "proj-1", "Blocked Task", NodeStatus::Blocked);

    graph_store.create_node(&completed_task).await?;
    graph_store.create_node(&pending_task).await?;
    let mut blocked_with_reason = blocked_task;
    blocked_with_reason.blocked_reason = Some("Waiting for external dependency".to_string());
    graph_store.create_node(&blocked_with_reason).await?;

    // Create a decision
    let decision = GraphNode {
        id: "ra-5678.4".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Decision,
        title: "Choice of Framework".to_string(),
        description: "Deciding which framework to use".to_string(),
        status: NodeStatus::Decided,
        priority: Some(Priority::High),
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: HashMap::new(),
    };
    graph_store.create_node(&decision).await?;

    // Create an option and link it as chosen
    let option = GraphNode {
        id: "ra-5678.5".to_string(),
        project_id: "proj-1".to_string(),
        node_type: NodeType::Option,
        title: "Use Rust".to_string(),
        description: "Using Rust for performance".to_string(),
        status: NodeStatus::Chosen,
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
    graph_store.create_node(&option).await?;

    // Add all edges
    for (from, to) in &[
        ("ra-5678", "ra-5678.1"),
        ("ra-5678", "ra-5678.2"),
        ("ra-5678", "ra-5678.3"),
        ("ra-5678", "ra-5678.4"),
        ("ra-5678.4", "ra-5678.5"),
    ] {
        let edge_type = if from.ends_with(".4") && to.ends_with(".5") {
            EdgeType::Chosen
        } else {
            EdgeType::Contains
        };

        graph_store
            .add_edge(&GraphEdge {
                id: generate_edge_id(),
                edge_type,
                from_node: from.to_string(),
                to_node: to.to_string(),
                label: if edge_type == EdgeType::Chosen {
                    Some("Best option for this use case".to_string())
                } else {
                    None
                },
                created_at: Utc::now(),
            })
            .await?;
    }

    // Create and end session
    let session = session_store.create_session("proj-1", "ra-5678").await?;

    session_store.end_session(&session.id, &graph_store).await?;

    let ended_session = session_store
        .get_session(&session.id)
        .await?
        .expect("session should exist");

    let notes = ended_session
        .handoff_notes
        .expect("handoff_notes should be present");

    // Verify all sections exist
    assert!(notes.contains("## Done"), "Should have 'Done' section");
    assert!(
        notes.contains("## Remaining"),
        "Should have 'Remaining' section"
    );
    assert!(
        notes.contains("## Blocked"),
        "Should have 'Blocked' section"
    );
    assert!(
        notes.contains("## Decisions Made"),
        "Should have 'Decisions Made' section"
    );

    // Verify content
    assert!(
        notes.contains("Completed Task"),
        "Done section should contain completed task"
    );
    assert!(
        notes.contains("Pending Task") || notes.contains("ra-5678.2"),
        "Remaining section should contain pending task"
    );
    assert!(
        notes.contains("Blocked Task"),
        "Blocked section should contain blocked task"
    );
    assert!(
        notes.contains("Waiting for external dependency"),
        "Blocked task should show reason"
    );
    assert!(
        notes.contains("Use Rust") || notes.contains("Choice of Framework"),
        "Decisions Made section should reference chosen option"
    );

    Ok(())
}

#[tokio::test]
async fn test_ac1_4_get_latest_session_returns_most_recent() -> Result<()> {
    // P1c.AC1.4: get_latest_session(goal_id) returns the most recent session
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal
    let goal = create_test_goal("ra-9999", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create two sessions for the same goal
    let session1 = session_store.create_session("proj-1", "ra-9999").await?;

    // Add a small delay to ensure different timestamps
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let session2 = session_store.create_session("proj-1", "ra-9999").await?;

    // Get the latest session
    let latest = session_store
        .get_latest_session("ra-9999")
        .await?
        .expect("latest session should exist");

    // Should be session2 (most recent)
    assert_eq!(latest.id, session2.id);
    assert_ne!(latest.id, session1.id);

    // Verify session1 still exists
    let first = session_store
        .get_session(&session1.id)
        .await?
        .expect("first session should still exist");
    assert_eq!(first.id, session1.id);

    Ok(())
}

#[tokio::test]
async fn test_list_sessions() -> Result<()> {
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal
    let goal = create_test_goal("ra-list-test", "proj-1", "Test Goal");
    graph_store.create_node(&goal).await?;

    // Create multiple sessions for the same goal
    let _session1 = session_store
        .create_session("proj-1", "ra-list-test")
        .await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let _session2 = session_store
        .create_session("proj-1", "ra-list-test")
        .await?;

    // List sessions
    let sessions = session_store.list_sessions("ra-list-test").await?;

    assert_eq!(sessions.len(), 2);
    // Should be in descending order by start time
    assert!(sessions[0].started_at >= sessions[1].started_at);

    Ok(())
}

#[tokio::test]
async fn test_handoff_notes_with_no_tasks() -> Result<()> {
    // Ensure handoff notes work even with no tasks
    let (db, graph_store) = common::setup_test_env().await?;
    let session_store = SessionStore::new(db);

    // Create a goal with no tasks
    let goal = create_test_goal("ra-empty", "proj-1", "Empty Goal");
    graph_store.create_node(&goal).await?;

    let session = session_store.create_session("proj-1", "ra-empty").await?;

    session_store.end_session(&session.id, &graph_store).await?;

    let ended_session = session_store
        .get_session(&session.id)
        .await?
        .expect("session should exist");

    let notes = ended_session
        .handoff_notes
        .expect("handoff_notes should be present");

    // Should have all sections but with "(none)" entries
    assert!(notes.contains("## Done"));
    assert!(notes.contains("## Remaining"));
    assert!(notes.contains("## Blocked"));
    assert!(notes.contains("## Decisions Made"));

    Ok(())
}
