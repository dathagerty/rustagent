//! Tests for node decay functionality (P1c.AC4.1 - P1c.AC4.4)

use chrono::{Duration, Utc};
use rustagent::graph::decay::{decay_node, decay_nodes, DecayConfig, DecayDetail};
use rustagent::graph::{GraphNode, NodeStatus, NodeType};
use std::collections::HashMap;

fn create_test_node(
    id: &str,
    created_days_ago: i64,
    completed_days_ago: Option<i64>,
) -> GraphNode {
    let now = Utc::now();
    let created_at = now - Duration::days(created_days_ago);
    let completed_at = completed_days_ago.map(|d| now - Duration::days(d));

    GraphNode {
        id: id.to_string(),
        project_id: "proj-test".to_string(),
        node_type: NodeType::Task,
        title: format!("Task {}", id),
        description: "Test description with important details".to_string(),
        status: NodeStatus::Completed,
        priority: None,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at,
        started_at: None,
        completed_at,
        blocked_reason: None,
        metadata: {
            let mut m = HashMap::new();
            m.insert("key_outcome".to_string(), "Important result".to_string());
            m.insert("context".to_string(), "Additional context".to_string());
            m
        },
    }
}

/// P1c.AC4.1: Nodes < 7 days old show Full detail
#[test]
fn test_full_detail_recent_node() {
    let config = DecayConfig::default();
    let node = create_test_node("recent", 10, Some(2)); // completed 2 days ago

    let decayed = decay_node(&node, Utc::now(), &config);

    assert_eq!(decayed.id, "recent");
    assert_eq!(decayed.title, "Task recent");
    assert_eq!(decayed.status, NodeStatus::Completed);

    match decayed.detail {
        DecayDetail::Full {
            description,
            metadata,
        } => {
            assert_eq!(description, "Test description with important details");
            assert_eq!(metadata.get("key_outcome"), Some(&"Important result".to_string()));
            assert_eq!(metadata.get("context"), Some(&"Additional context".to_string()));
        }
        other => panic!("Expected Full detail for recent node, got {:?}", other),
    }
}

/// P1c.AC4.2: Nodes 7-30 days old show Summary only
#[test]
fn test_summary_detail_older_node() {
    let config = DecayConfig::default();
    let node = create_test_node("older", 20, Some(15)); // completed 15 days ago

    let decayed = decay_node(&node, Utc::now(), &config);

    assert_eq!(decayed.id, "older");
    assert_eq!(decayed.title, "Task older");
    assert_eq!(decayed.status, NodeStatus::Completed);

    match decayed.detail {
        DecayDetail::Summary { key_outcome } => {
            assert_eq!(key_outcome, Some("Important result".to_string()));
        }
        other => panic!("Expected Summary detail for older node, got {:?}", other),
    }
}

/// P1c.AC4.3: Nodes > 30 days old show Minimal detail
#[test]
fn test_minimal_detail_very_old_node() {
    let config = DecayConfig::default();
    let node = create_test_node("very_old", 50, Some(45)); // completed 45 days ago

    let decayed = decay_node(&node, Utc::now(), &config);

    assert_eq!(decayed.id, "very_old");
    assert_eq!(decayed.title, "Task very_old");
    assert_eq!(decayed.status, NodeStatus::Completed);

    match decayed.detail {
        DecayDetail::Minimal => {
            // Expected - no additional fields
        }
        other => panic!("Expected Minimal detail for very old node, got {:?}", other),
    }
}

/// P1c.AC4.4: Configurable thresholds work correctly
#[test]
fn test_configurable_thresholds() {
    // Custom config: full up to 3 days, summary up to 10 days
    let config = DecayConfig {
        recent_days: 3,
        older_days: 10,
    };

    let now = Utc::now();

    // Node completed 2 days ago - should be Full with custom config
    let node_recent = create_test_node("n_recent", 10, Some(2));
    let decayed_recent = decay_node(&node_recent, now, &config);
    assert!(
        matches!(decayed_recent.detail, DecayDetail::Full { .. }),
        "2 days old should be Full with recent_days=3"
    );

    // Node completed 5 days ago - should be Summary with custom config
    let node_mid = create_test_node("n_mid", 10, Some(5));
    let decayed_mid = decay_node(&node_mid, now, &config);
    assert!(
        matches!(decayed_mid.detail, DecayDetail::Summary { .. }),
        "5 days old should be Summary with recent_days=3, older_days=10"
    );

    // Node completed 12 days ago - should be Minimal with custom config
    let node_old = create_test_node("n_old", 15, Some(12));
    let decayed_old = decay_node(&node_old, now, &config);
    assert!(
        matches!(decayed_old.detail, DecayDetail::Minimal),
        "12 days old should be Minimal with older_days=10"
    );
}

/// Test decay_nodes applies decay to multiple nodes
#[test]
fn test_decay_multiple_nodes() {
    let config = DecayConfig::default();
    let nodes = vec![
        create_test_node("n1", 10, Some(2)),   // Full
        create_test_node("n2", 20, Some(15)),  // Summary
        create_test_node("n3", 50, Some(45)),  // Minimal
    ];

    let decayed = decay_nodes(&nodes, Utc::now(), &config);

    assert_eq!(decayed.len(), 3);

    assert!(matches!(decayed[0].detail, DecayDetail::Full { .. }));
    assert_eq!(decayed[0].id, "n1");

    assert!(matches!(decayed[1].detail, DecayDetail::Summary { .. }));
    assert_eq!(decayed[1].id, "n2");

    assert!(matches!(decayed[2].detail, DecayDetail::Minimal));
    assert_eq!(decayed[2].id, "n3");
}

/// Test that completed_at is preferred over created_at for age calculation
#[test]
fn test_uses_completed_time_for_age() {
    let config = DecayConfig::default();
    let now = Utc::now();

    // Node created 100 days ago but completed 2 days ago should be Full
    let node = create_test_node("old_created", 100, Some(2));
    let decayed = decay_node(&node, now, &config);

    match decayed.detail {
        DecayDetail::Full { .. } => {
            // Expected - uses completed_at (2 days old), not created_at (100 days old)
        }
        other => panic!(
            "Should use completed_at for age: got {:?}",
            other
        ),
    }
}

/// Test decay of node without completion time
#[test]
fn test_decay_node_without_completion() {
    let config = DecayConfig::default();
    let mut node = create_test_node("in_progress", 2, None);
    node.status = NodeStatus::InProgress;

    let decayed = decay_node(&node, Utc::now(), &config);

    // Should use created_at - 2 days old, so Full
    match decayed.detail {
        DecayDetail::Full { .. } => {
            // Expected - uses created_at when completed_at is None
        }
        other => panic!("Should use created_at when completed_at is None: got {:?}", other),
    }
}

/// Test summary without key_outcome in metadata
#[test]
fn test_summary_without_key_outcome() {
    let config = DecayConfig::default();
    let mut node = create_test_node("no_outcome", 10, Some(15));
    node.metadata.remove("key_outcome");

    let decayed = decay_node(&node, Utc::now(), &config);

    match decayed.detail {
        DecayDetail::Summary { key_outcome } => {
            assert_eq!(key_outcome, None);
        }
        other => panic!("Expected Summary without key_outcome: got {:?}", other),
    }
}

/// Test boundary condition at exact threshold (7 days = Summary)
#[test]
fn test_boundary_recent_to_summary() {
    let config = DecayConfig::default();
    let now = Utc::now();

    // Over 7 days ago should transition to Summary (not Full)
    let node = create_test_node("boundary_7", 10, Some(8));
    let decayed = decay_node(&node, now, &config);

    assert!(
        matches!(decayed.detail, DecayDetail::Summary { .. }),
        "Node over 7 days old should be Summary"
    );
}

/// Test boundary condition at exact threshold (30 days = Minimal)
#[test]
fn test_boundary_summary_to_minimal() {
    let config = DecayConfig::default();
    let now = Utc::now();

    // Over 30 days ago should be Minimal
    let node = create_test_node("boundary_30", 40, Some(31));
    let decayed = decay_node(&node, now, &config);

    assert!(
        matches!(decayed.detail, DecayDetail::Minimal),
        "Node over 30 days old should be Minimal"
    );
}

/// Test that all metadata fields are preserved in Full detail
#[test]
fn test_full_detail_preserves_all_metadata() {
    let config = DecayConfig::default();
    let mut node = create_test_node("full_meta", 10, Some(1));

    node.metadata.insert("custom_field".to_string(), "custom_value".to_string());
    node.metadata.insert("tags".to_string(), "a,b,c".to_string());

    let decayed = decay_node(&node, Utc::now(), &config);

    match decayed.detail {
        DecayDetail::Full {
            description: _,
            metadata,
        } => {
            assert_eq!(metadata.len(), 4);
            assert_eq!(metadata.get("custom_field"), Some(&"custom_value".to_string()));
            assert_eq!(metadata.get("tags"), Some(&"a,b,c".to_string()));
        }
        other => panic!("Expected Full detail with all metadata: got {:?}", other),
    }
}
