use chrono::Utc;
use rustagent::graph::*;
use std::collections::HashMap;

#[test]
fn test_node_type_all_variants_roundtrip() {
    let types = vec![
        NodeType::Goal,
        NodeType::Task,
        NodeType::Decision,
        NodeType::Option,
        NodeType::Outcome,
        NodeType::Observation,
        NodeType::Revisit,
    ];

    for node_type in types {
        let string_repr = node_type.to_string();
        let parsed: NodeType = string_repr.parse().expect("Failed to parse");
        assert_eq!(node_type, parsed, "Roundtrip failed for {:?}", node_type);
    }
}

#[test]
fn test_edge_type_all_variants_roundtrip() {
    let types = vec![
        EdgeType::Contains,
        EdgeType::DependsOn,
        EdgeType::LeadsTo,
        EdgeType::Chosen,
        EdgeType::Rejected,
        EdgeType::Supersedes,
        EdgeType::Informs,
    ];

    for edge_type in types {
        let string_repr = edge_type.to_string();
        let parsed: EdgeType = string_repr.parse().expect("Failed to parse");
        assert_eq!(edge_type, parsed, "Roundtrip failed for {:?}", edge_type);
    }
}

#[test]
fn test_validate_status_task_ready() {
    let result = validate_status(&NodeType::Task, &NodeStatus::Ready);
    assert!(result.is_ok(), "Task should allow Ready status");
}

#[test]
fn test_validate_status_goal_ready_invalid() {
    let result = validate_status(&NodeType::Goal, &NodeStatus::Ready);
    assert!(result.is_err(), "Goal should not allow Ready status");
}

#[test]
fn test_node_status_all_variants_roundtrip() {
    let statuses = vec![
        NodeStatus::Pending,
        NodeStatus::Active,
        NodeStatus::Completed,
        NodeStatus::Cancelled,
        NodeStatus::Ready,
        NodeStatus::Claimed,
        NodeStatus::InProgress,
        NodeStatus::Review,
        NodeStatus::Blocked,
        NodeStatus::Failed,
        NodeStatus::Decided,
        NodeStatus::Superseded,
        NodeStatus::Abandoned,
        NodeStatus::Chosen,
        NodeStatus::Rejected,
    ];

    for status in statuses {
        let string_repr = status.to_string();
        let parsed: NodeStatus = string_repr.parse().expect("Failed to parse");
        assert_eq!(status, parsed, "Roundtrip failed for {:?}", status);
    }
}

#[test]
fn test_graph_node_serialization_and_deserialization() {
    let mut metadata = HashMap::new();
    metadata.insert("key1".to_string(), "value1".to_string());

    let node = GraphNode {
        id: "ra-a3f8".to_string(),
        project_id: "proj-001".to_string(),
        node_type: NodeType::Task,
        title: "Complete Feature X".to_string(),
        description: "Implement feature X with proper error handling".to_string(),
        status: NodeStatus::Active,
        priority: Some(Priority::High),
        assigned_to: Some("alice@example.com".to_string()),
        created_by: Some("bob@example.com".to_string()),
        labels: vec!["feature".to_string(), "high-priority".to_string()],
        created_at: Utc::now(),
        started_at: Some(Utc::now()),
        completed_at: None,
        blocked_reason: None,
        metadata,
    };

    let json = serde_json::to_string(&node).expect("Serialization failed");
    let deserialized: GraphNode = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(node.id, deserialized.id);
    assert_eq!(node.project_id, deserialized.project_id);
    assert_eq!(node.node_type, deserialized.node_type);
    assert_eq!(node.title, deserialized.title);
    assert_eq!(node.description, deserialized.description);
    assert_eq!(node.status, deserialized.status);
    assert_eq!(node.priority, deserialized.priority);
    assert_eq!(node.assigned_to, deserialized.assigned_to);
    assert_eq!(node.created_by, deserialized.created_by);
    assert_eq!(node.labels, deserialized.labels);
}

#[test]
fn test_graph_edge_serialization_and_deserialization() {
    let edge = GraphEdge {
        id: "e-12345678".to_string(),
        edge_type: EdgeType::DependsOn,
        from_node: "ra-a3f8.1".to_string(),
        to_node: "ra-a3f8.2".to_string(),
        label: Some("task_depends_on".to_string()),
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&edge).expect("Serialization failed");
    let deserialized: GraphEdge = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(edge.id, deserialized.id);
    assert_eq!(edge.edge_type, deserialized.edge_type);
    assert_eq!(edge.from_node, deserialized.from_node);
    assert_eq!(edge.to_node, deserialized.to_node);
    assert_eq!(edge.label, deserialized.label);
}

#[test]
fn test_generate_goal_id_format() {
    let id = generate_goal_id();
    assert!(id.starts_with("ra-"), "Goal ID should start with 'ra-'");
    assert_eq!(id.len(), 7, "Goal ID should be 7 characters (ra- + 4 hex)");

    // Verify it's valid hex after the prefix
    let hex_part = &id[3..];
    assert!(
        u32::from_str_radix(hex_part, 16).is_ok(),
        "ID suffix should be valid hex"
    );
}

#[test]
fn test_generate_goal_id_uniqueness() {
    let id1 = generate_goal_id();
    let id2 = generate_goal_id();
    assert_ne!(id1, id2, "Generated IDs should be unique");
}

#[test]
fn test_generate_child_id_format() {
    let parent = "ra-a3f8";
    let child = generate_child_id(parent, 1);
    assert_eq!(child, "ra-a3f8.1");

    let grandchild = generate_child_id(&child, 3);
    assert_eq!(grandchild, "ra-a3f8.1.3");

    let great_grandchild = generate_child_id(&grandchild, 2);
    assert_eq!(great_grandchild, "ra-a3f8.1.3.2");
}

#[test]
fn test_generate_child_id_various_sequences() {
    for seq in 1..=100 {
        let child = generate_child_id("ra-a3f8", seq);
        assert_eq!(child, format!("ra-a3f8.{}", seq));
    }
}

#[test]
fn test_generate_edge_id_format() {
    let id = generate_edge_id();
    assert!(id.starts_with("e-"), "Edge ID should start with 'e-'");
    assert_eq!(id.len(), 10, "Edge ID should be 10 characters (e- + 8 hex)");

    // Verify it's valid hex after the prefix
    let hex_part = &id[2..];
    assert!(
        u32::from_str_radix(hex_part, 16).is_ok(),
        "Edge ID suffix should be valid hex"
    );
}

#[test]
fn test_generate_edge_id_uniqueness() {
    let id1 = generate_edge_id();
    let id2 = generate_edge_id();
    assert_ne!(id1, id2, "Generated edge IDs should be unique");
}

#[test]
fn test_parent_id_extraction_single_level() {
    let result = parent_id("ra-a3f8.1");
    assert_eq!(result, Some("ra-a3f8"));
}

#[test]
fn test_parent_id_extraction_multiple_levels() {
    assert_eq!(parent_id("ra-a3f8.1.3"), Some("ra-a3f8.1"));
    assert_eq!(parent_id("ra-a3f8.1.3.2"), Some("ra-a3f8.1.3"));
}

#[test]
fn test_parent_id_extraction_root_returns_none() {
    assert_eq!(parent_id("ra-a3f8"), None);
}

#[test]
fn test_parent_id_extraction_edge_ids() {
    // Edge IDs should not have extractable parents (no dots)
    assert_eq!(parent_id("e-12345678"), None);
}

#[test]
fn test_hierarchical_id_path_building() {
    let root = generate_goal_id();
    let child1 = generate_child_id(&root, 1);
    let child2 = generate_child_id(&child1, 1);
    let child3 = generate_child_id(&child2, 1);

    // Verify the chain
    assert_eq!(parent_id(&child1), Some(root.as_str()));
    assert_eq!(parent_id(&child2), Some(child1.as_str()));
    assert_eq!(parent_id(&child3), Some(child2.as_str()));
}

#[test]
fn test_priority_all_variants() {
    let priorities = vec![
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ];

    for priority in priorities {
        let string_repr = priority.to_string();
        let parsed: Priority = string_repr.parse().expect("Failed to parse");
        assert_eq!(priority, parsed, "Roundtrip failed for {:?}", priority);
    }
}

#[test]
fn test_valid_statuses_for_all_node_types() {
    // Goal: Pending, Active, Completed, Cancelled
    let goal_statuses = valid_statuses(&NodeType::Goal);
    assert!(goal_statuses.contains(&NodeStatus::Pending));
    assert!(goal_statuses.contains(&NodeStatus::Active));
    assert!(goal_statuses.contains(&NodeStatus::Completed));
    assert!(goal_statuses.contains(&NodeStatus::Cancelled));
    assert!(!goal_statuses.contains(&NodeStatus::Ready));

    // Task: Pending, Ready, Claimed, InProgress, Review, Completed, Blocked, Failed, Cancelled
    let task_statuses = valid_statuses(&NodeType::Task);
    assert!(task_statuses.contains(&NodeStatus::Ready));
    assert!(task_statuses.contains(&NodeStatus::Claimed));
    assert!(task_statuses.contains(&NodeStatus::InProgress));
    assert!(!task_statuses.contains(&NodeStatus::Decided));

    // Decision: Pending, Active, Decided, Superseded
    let decision_statuses = valid_statuses(&NodeType::Decision);
    assert!(decision_statuses.contains(&NodeStatus::Decided));
    assert!(decision_statuses.contains(&NodeStatus::Superseded));
    assert!(!decision_statuses.contains(&NodeStatus::Ready));

    // Option: Pending, Active, Chosen, Rejected, Abandoned
    let option_statuses = valid_statuses(&NodeType::Option);
    assert!(option_statuses.contains(&NodeStatus::Chosen));
    assert!(option_statuses.contains(&NodeStatus::Rejected));
    assert!(option_statuses.contains(&NodeStatus::Abandoned));

    // Outcome: Active, Completed
    let outcome_statuses = valid_statuses(&NodeType::Outcome);
    assert_eq!(outcome_statuses.len(), 2);
    assert!(outcome_statuses.contains(&NodeStatus::Active));
    assert!(outcome_statuses.contains(&NodeStatus::Completed));

    // Observation: Active
    let observation_statuses = valid_statuses(&NodeType::Observation);
    assert_eq!(observation_statuses.len(), 1);
    assert!(observation_statuses.contains(&NodeStatus::Active));

    // Revisit: Active, Completed
    let revisit_statuses = valid_statuses(&NodeType::Revisit);
    assert_eq!(revisit_statuses.len(), 2);
    assert!(revisit_statuses.contains(&NodeStatus::Active));
    assert!(revisit_statuses.contains(&NodeStatus::Completed));
}
