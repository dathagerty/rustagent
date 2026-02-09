//! Node decay for context injection based on age thresholds.
//!
//! This module provides functions to "decay" node details based on how old they are,
//! reducing detail for old nodes to save context tokens when injecting nodes into LLM prompts.
//!
//! # Decay Levels
//!
//! - **Full** (< 7 days): title, description, status, metadata
//! - **Summary** (7-30 days): title, status, key outcome from metadata
//! - **Minimal** (> 30 days): title and status only
//!
//! Thresholds are configurable via `DecayConfig`.

use crate::graph::{GraphNode, NodeStatus};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Configuration for node decay thresholds
#[derive(Debug, Clone)]
pub struct DecayConfig {
    /// Days before recent threshold (default: 7). Nodes older than this but
    /// younger than `older_days` show Summary detail.
    pub recent_days: i64,

    /// Days before old threshold (default: 30). Nodes older than this show
    /// Minimal detail.
    pub older_days: i64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            recent_days: 7,
            older_days: 30,
        }
    }
}

/// Severity level of node detail
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecayLevel {
    /// Full detail: description and metadata included
    Full,
    /// Summary: title, status, key outcome only
    Summary,
    /// Minimal: title and status only
    Minimal,
}

/// Detailed information for a decayed node
#[derive(Debug, Clone)]
pub enum DecayDetail {
    /// Full detail with description and metadata
    Full {
        /// Node description
        description: String,
        /// Additional metadata
        metadata: HashMap<String, String>,
    },
    /// Summary with key outcome extracted from metadata
    Summary {
        /// Key outcome if present in metadata (under "key_outcome" field)
        key_outcome: Option<String>,
    },
    /// Minimal detail - no additional fields
    Minimal,
}

/// A graph node with decayed details based on age
#[derive(Debug, Clone)]
pub struct DecayedNode {
    /// Node ID
    pub id: String,
    /// Node title
    pub title: String,
    /// Node status
    pub status: NodeStatus,
    /// Detail level based on age
    pub detail: DecayDetail,
}

/// Determine the decay level for a node based on its age
fn decay_level_for_age(age_days: i64, config: &DecayConfig) -> DecayLevel {
    if age_days < config.recent_days {
        DecayLevel::Full
    } else if age_days < config.older_days {
        DecayLevel::Summary
    } else {
        DecayLevel::Minimal
    }
}

/// Calculate the age of a node in days
///
/// Uses `completed_at` if available, otherwise `created_at`. Returns the number
/// of complete days between the node's reference date and the provided `now`.
fn node_age_days(node: &GraphNode, now: DateTime<Utc>) -> i64 {
    let reference_time = node.completed_at.unwrap_or(node.created_at);
    let duration = now.signed_duration_since(reference_time);
    duration.num_days()
}

/// Apply decay to a single node
///
/// Computes the node's age and selects a decay level. Returns a `DecayedNode`
/// with details appropriate to the age threshold.
///
/// # Arguments
///
/// * `node` - The graph node to decay
/// * `now` - Current time for age calculation
/// * `config` - Decay configuration with thresholds
///
/// # Example
///
/// ```ignore
/// let config = DecayConfig::default();
/// let decayed = decay_node(&node, Utc::now(), &config);
/// match decayed.detail {
///     DecayDetail::Full { .. } => println!("Recent node"),
///     DecayDetail::Summary { .. } => println!("Older node"),
///     DecayDetail::Minimal => println!("Very old node"),
/// }
/// ```
pub fn decay_node(node: &GraphNode, now: DateTime<Utc>, config: &DecayConfig) -> DecayedNode {
    let age_days = node_age_days(node, now);
    let level = decay_level_for_age(age_days, config);

    let detail = match level {
        DecayLevel::Full => DecayDetail::Full {
            description: node.description.clone(),
            metadata: node.metadata.clone(),
        },
        DecayLevel::Summary => {
            let key_outcome = node.metadata.get("key_outcome").cloned();
            DecayDetail::Summary { key_outcome }
        }
        DecayLevel::Minimal => DecayDetail::Minimal,
    };

    DecayedNode {
        id: node.id.clone(),
        title: node.title.clone(),
        status: node.status,
        detail,
    }
}

/// Apply decay to multiple nodes
///
/// Applies `decay_node` to each node in the slice and returns a vector
/// of decayed nodes.
///
/// # Arguments
///
/// * `nodes` - Slice of graph nodes to decay
/// * `now` - Current time for age calculation
/// * `config` - Decay configuration with thresholds
pub fn decay_nodes(
    nodes: &[GraphNode],
    now: DateTime<Utc>,
    config: &DecayConfig,
) -> Vec<DecayedNode> {
    nodes.iter().map(|n| decay_node(n, now, config)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn create_test_node(id: &str, created_days_ago: i64, completed_days_ago: Option<i64>) -> GraphNode {
        let now = Utc::now();
        let created_at = now - Duration::days(created_days_ago);
        let completed_at = completed_days_ago.map(|d| now - Duration::days(d));

        GraphNode {
            id: id.to_string(),
            project_id: "proj-test".to_string(),
            node_type: crate::graph::NodeType::Task,
            title: format!("Task {}", id),
            description: "Test description".to_string(),
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
                m
            },
        }
    }

    #[test]
    fn test_decay_full_detail_recent_node() {
        let config = DecayConfig::default();
        let node = create_test_node("n1", 10, Some(2)); // created 10 days ago, completed 2 days ago
        let decayed = decay_node(&node, Utc::now(), &config);

        assert_eq!(decayed.id, "n1");
        assert_eq!(decayed.title, "Task n1");
        assert_eq!(decayed.status, NodeStatus::Completed);

        match decayed.detail {
            DecayDetail::Full {
                description,
                metadata,
            } => {
                assert_eq!(description, "Test description");
                assert!(metadata.contains_key("key_outcome"));
            }
            _ => panic!("Expected Full detail for recent node"),
        }
    }

    #[test]
    fn test_decay_summary_older_node() {
        let config = DecayConfig::default();
        let node = create_test_node("n2", 20, Some(15)); // completed 15 days ago
        let decayed = decay_node(&node, Utc::now(), &config);

        assert_eq!(decayed.id, "n2");
        assert_eq!(decayed.status, NodeStatus::Completed);

        match decayed.detail {
            DecayDetail::Summary { key_outcome } => {
                assert_eq!(key_outcome, Some("Important result".to_string()));
            }
            _ => panic!("Expected Summary detail for older node"),
        }
    }

    #[test]
    fn test_decay_minimal_very_old_node() {
        let config = DecayConfig::default();
        let node = create_test_node("n3", 50, Some(45)); // completed 45 days ago
        let decayed = decay_node(&node, Utc::now(), &config);

        assert_eq!(decayed.id, "n3");
        assert_eq!(decayed.status, NodeStatus::Completed);

        match decayed.detail {
            DecayDetail::Minimal => {
                // Expected
            }
            _ => panic!("Expected Minimal detail for very old node"),
        }
    }

    #[test]
    fn test_decay_custom_config() {
        let config = DecayConfig {
            recent_days: 3,
            older_days: 10,
        };

        let node = create_test_node("n4", 10, Some(5)); // completed 5 days ago
        let decayed = decay_node(&node, Utc::now(), &config);

        // With custom config, 5 days ago is between 3 and 10, so should be Summary
        match decayed.detail {
            DecayDetail::Summary { .. } => {
                // Expected
            }
            _ => panic!("Expected Summary detail with custom config"),
        }
    }

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
        assert!(matches!(decayed[1].detail, DecayDetail::Summary { .. }));
        assert!(matches!(decayed[2].detail, DecayDetail::Minimal));
    }

    #[test]
    fn test_decay_uses_completed_time_if_available() {
        let config = DecayConfig::default();
        let now = Utc::now();

        // Node created 100 days ago but completed 2 days ago should be Full
        let node = create_test_node("n5", 100, Some(2));
        let decayed = decay_node(&node, now, &config);

        match decayed.detail {
            DecayDetail::Full { .. } => {
                // Expected - uses completed_at (2 days old)
            }
            _ => panic!("Should use completed_at for age calculation"),
        }
    }

    #[test]
    fn test_decay_node_without_completion() {
        let config = DecayConfig::default();
        let mut node = create_test_node("n6", 2, None); // No completion time
        node.status = NodeStatus::InProgress;

        let decayed = decay_node(&node, Utc::now(), &config);

        // Should use created_at - 2 days old, so Full
        match decayed.detail {
            DecayDetail::Full { .. } => {
                // Expected
            }
            _ => panic!("Should use created_at when completed_at is None"),
        }
    }

    #[test]
    fn test_summary_without_key_outcome() {
        let config = DecayConfig::default();
        let mut node = create_test_node("n7", 10, Some(15));
        node.metadata.clear(); // Remove key_outcome

        let decayed = decay_node(&node, Utc::now(), &config);

        match decayed.detail {
            DecayDetail::Summary { key_outcome } => {
                assert_eq!(key_outcome, None);
            }
            _ => panic!("Expected Summary without key_outcome"),
        }
    }
}
