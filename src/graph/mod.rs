use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

pub mod dependency;
pub mod store;

/// Node type in the work graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Goal,
    Task,
    Decision,
    Option,
    Outcome,
    Observation,
    Revisit,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Goal => write!(f, "goal"),
            NodeType::Task => write!(f, "task"),
            NodeType::Decision => write!(f, "decision"),
            NodeType::Option => write!(f, "option"),
            NodeType::Outcome => write!(f, "outcome"),
            NodeType::Observation => write!(f, "observation"),
            NodeType::Revisit => write!(f, "revisit"),
        }
    }
}

impl FromStr for NodeType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "goal" => Ok(NodeType::Goal),
            "task" => Ok(NodeType::Task),
            "decision" => Ok(NodeType::Decision),
            "option" => Ok(NodeType::Option),
            "outcome" => Ok(NodeType::Outcome),
            "observation" => Ok(NodeType::Observation),
            "revisit" => Ok(NodeType::Revisit),
            _ => Err(anyhow!("Unknown node type: {}", s)),
        }
    }
}

/// Edge type connecting nodes in the work graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeType {
    Contains,
    DependsOn,
    LeadsTo,
    Chosen,
    Rejected,
    Supersedes,
    Informs,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Contains => write!(f, "contains"),
            EdgeType::DependsOn => write!(f, "depends_on"),
            EdgeType::LeadsTo => write!(f, "leads_to"),
            EdgeType::Chosen => write!(f, "chosen"),
            EdgeType::Rejected => write!(f, "rejected"),
            EdgeType::Supersedes => write!(f, "supersedes"),
            EdgeType::Informs => write!(f, "informs"),
        }
    }
}

impl FromStr for EdgeType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "contains" => Ok(EdgeType::Contains),
            "depends_on" | "dependson" => Ok(EdgeType::DependsOn),
            "leads_to" | "leadsto" => Ok(EdgeType::LeadsTo),
            "chosen" => Ok(EdgeType::Chosen),
            "rejected" => Ok(EdgeType::Rejected),
            "supersedes" => Ok(EdgeType::Supersedes),
            "informs" => Ok(EdgeType::Informs),
            _ => Err(anyhow!("Unknown edge type: {}", s)),
        }
    }
}

/// Status of a node in the work graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    // Lifecycle (all node types)
    Pending,
    Active,
    Completed,
    Cancelled,

    // Task workflow
    Ready,
    Claimed,
    InProgress,
    Review,
    Blocked,
    Failed,

    // Decision workflow
    Decided,
    Superseded,
    Abandoned,

    // Option workflow
    Chosen,
    Rejected,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Pending => write!(f, "pending"),
            NodeStatus::Active => write!(f, "active"),
            NodeStatus::Completed => write!(f, "completed"),
            NodeStatus::Cancelled => write!(f, "cancelled"),
            NodeStatus::Ready => write!(f, "ready"),
            NodeStatus::Claimed => write!(f, "claimed"),
            NodeStatus::InProgress => write!(f, "in_progress"),
            NodeStatus::Review => write!(f, "review"),
            NodeStatus::Blocked => write!(f, "blocked"),
            NodeStatus::Failed => write!(f, "failed"),
            NodeStatus::Decided => write!(f, "decided"),
            NodeStatus::Superseded => write!(f, "superseded"),
            NodeStatus::Abandoned => write!(f, "abandoned"),
            NodeStatus::Chosen => write!(f, "chosen"),
            NodeStatus::Rejected => write!(f, "rejected"),
        }
    }
}

impl FromStr for NodeStatus {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(NodeStatus::Pending),
            "active" => Ok(NodeStatus::Active),
            "completed" => Ok(NodeStatus::Completed),
            "cancelled" => Ok(NodeStatus::Cancelled),
            "ready" => Ok(NodeStatus::Ready),
            "claimed" => Ok(NodeStatus::Claimed),
            "in_progress" | "inprogress" => Ok(NodeStatus::InProgress),
            "review" => Ok(NodeStatus::Review),
            "blocked" => Ok(NodeStatus::Blocked),
            "failed" => Ok(NodeStatus::Failed),
            "decided" => Ok(NodeStatus::Decided),
            "superseded" => Ok(NodeStatus::Superseded),
            "abandoned" => Ok(NodeStatus::Abandoned),
            "chosen" => Ok(NodeStatus::Chosen),
            "rejected" => Ok(NodeStatus::Rejected),
            _ => Err(anyhow!("Unknown node status: {}", s)),
        }
    }
}

/// Priority level for a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Critical => write!(f, "critical"),
            Priority::High => write!(f, "high"),
            Priority::Medium => write!(f, "medium"),
            Priority::Low => write!(f, "low"),
        }
    }
}

impl FromStr for Priority {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Priority::Critical),
            "high" => Ok(Priority::High),
            "medium" => Ok(Priority::Medium),
            "low" => Ok(Priority::Low),
            _ => Err(anyhow!("Unknown priority: {}", s)),
        }
    }
}

/// A node in the work graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub project_id: String,
    pub node_type: NodeType,
    pub title: String,
    pub description: String,
    pub status: NodeStatus,
    pub priority: Option<Priority>,
    pub assigned_to: Option<String>,
    pub created_by: Option<String>,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub blocked_reason: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// An edge connecting two nodes in the work graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub edge_type: EdgeType,
    pub from_node: String,
    pub to_node: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Returns the valid statuses for a given node type
pub fn valid_statuses(node_type: &NodeType) -> Vec<NodeStatus> {
    match node_type {
        NodeType::Goal => vec![
            NodeStatus::Pending,
            NodeStatus::Active,
            NodeStatus::Completed,
            NodeStatus::Cancelled,
        ],
        NodeType::Task => vec![
            NodeStatus::Pending,
            NodeStatus::Ready,
            NodeStatus::Claimed,
            NodeStatus::InProgress,
            NodeStatus::Review,
            NodeStatus::Completed,
            NodeStatus::Blocked,
            NodeStatus::Failed,
            NodeStatus::Cancelled,
        ],
        NodeType::Decision => vec![
            NodeStatus::Pending,
            NodeStatus::Active,
            NodeStatus::Decided,
            NodeStatus::Superseded,
        ],
        NodeType::Option => vec![
            NodeStatus::Pending,
            NodeStatus::Active,
            NodeStatus::Chosen,
            NodeStatus::Rejected,
            NodeStatus::Abandoned,
        ],
        NodeType::Outcome => vec![NodeStatus::Active, NodeStatus::Completed],
        NodeType::Observation => vec![NodeStatus::Active],
        NodeType::Revisit => vec![NodeStatus::Active, NodeStatus::Completed],
    }
}

/// Validates that a status is valid for a given node type
pub fn validate_status(node_type: &NodeType, status: &NodeStatus) -> Result<()> {
    if valid_statuses(node_type).contains(status) {
        Ok(())
    } else {
        Err(anyhow!(
            "Status {:?} is not valid for node type {:?}",
            status,
            node_type
        ))
    }
}

/// Generate a goal ID (ra-xxxx where xxxx is 4 hex chars from UUID v4)
pub fn generate_goal_id() -> String {
    format!("ra-{}", &uuid::Uuid::new_v4().simple().to_string()[..4])
}

/// Generate a child ID from a parent ID and sequence number
pub fn generate_child_id(parent_id: &str, seq: u32) -> String {
    format!("{}.{}", parent_id, seq)
}

/// Generate an edge ID (e-xxxxxxxx where xxxxxxxx is 8 hex chars from UUID v4)
pub fn generate_edge_id() -> String {
    format!("e-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// Extract the parent ID from a hierarchical ID
/// E.g., "ra-a3f8.1.3" -> Some("ra-a3f8.1")
/// "ra-a3f8" -> None
pub fn parent_id(id: &str) -> Option<&str> {
    if let Some(last_dot) = id.rfind('.') {
        Some(&id[..last_dot])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_display_and_fromstr() {
        let node_types = vec![
            NodeType::Goal,
            NodeType::Task,
            NodeType::Decision,
            NodeType::Option,
            NodeType::Outcome,
            NodeType::Observation,
            NodeType::Revisit,
        ];

        for nt in node_types {
            let s = nt.to_string();
            let parsed: NodeType = s.parse().expect("Failed to parse NodeType");
            assert_eq!(nt, parsed);
        }
    }

    #[test]
    fn test_edge_type_display_and_fromstr() {
        let edge_types = vec![
            EdgeType::Contains,
            EdgeType::DependsOn,
            EdgeType::LeadsTo,
            EdgeType::Chosen,
            EdgeType::Rejected,
            EdgeType::Supersedes,
            EdgeType::Informs,
        ];

        for et in edge_types {
            let s = et.to_string();
            let parsed: EdgeType = s.parse().expect("Failed to parse EdgeType");
            assert_eq!(et, parsed);
        }
    }

    #[test]
    fn test_node_status_display_and_fromstr() {
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
            let s = status.to_string();
            let parsed: NodeStatus = s.parse().expect("Failed to parse NodeStatus");
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_priority_display_and_fromstr() {
        let priorities = vec![
            Priority::Critical,
            Priority::High,
            Priority::Medium,
            Priority::Low,
        ];

        for priority in priorities {
            let s = priority.to_string();
            let parsed: Priority = s.parse().expect("Failed to parse Priority");
            assert_eq!(priority, parsed);
        }
    }

    #[test]
    fn test_valid_statuses_task() {
        let valid = valid_statuses(&NodeType::Task);
        assert!(valid.contains(&NodeStatus::Ready));
        assert!(valid.contains(&NodeStatus::Claimed));
        assert!(valid.contains(&NodeStatus::InProgress));
    }

    #[test]
    fn test_valid_statuses_goal() {
        let valid = valid_statuses(&NodeType::Goal);
        assert!(valid.contains(&NodeStatus::Pending));
        assert!(valid.contains(&NodeStatus::Active));
        assert!(valid.contains(&NodeStatus::Completed));
        assert!(!valid.contains(&NodeStatus::Ready));
    }

    #[test]
    fn test_validate_status_valid() {
        let result = validate_status(&NodeType::Task, &NodeStatus::Ready);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_status_invalid() {
        let result = validate_status(&NodeType::Goal, &NodeStatus::Ready);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_goal_id() {
        let id = generate_goal_id();
        assert!(id.starts_with("ra-"));
        assert_eq!(id.len(), 7); // "ra-" + 4 hex chars
    }

    #[test]
    fn test_generate_child_id() {
        let child = generate_child_id("ra-a3f8", 1);
        assert_eq!(child, "ra-a3f8.1");

        let grandchild = generate_child_id("ra-a3f8.1", 3);
        assert_eq!(grandchild, "ra-a3f8.1.3");
    }

    #[test]
    fn test_generate_edge_id() {
        let id = generate_edge_id();
        assert!(id.starts_with("e-"));
        assert_eq!(id.len(), 10); // "e-" + 8 hex chars
    }

    #[test]
    fn test_parent_id_extraction() {
        assert_eq!(parent_id("ra-a3f8.1.3"), Some("ra-a3f8.1"));
        assert_eq!(parent_id("ra-a3f8.1"), Some("ra-a3f8"));
        assert_eq!(parent_id("ra-a3f8"), None);
    }

    #[test]
    fn test_graph_node_serialization() {
        let node = GraphNode {
            id: "ra-a3f8".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Goal,
            title: "Test Goal".to_string(),
            description: "A test goal".to_string(),
            status: NodeStatus::Active,
            priority: Some(Priority::High),
            assigned_to: Some("user1".to_string()),
            created_by: Some("user2".to_string()),
            labels: vec!["label1".to_string()],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&node).expect("Failed to serialize");
        let deserialized: GraphNode = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(node.id, deserialized.id);
        assert_eq!(node.title, deserialized.title);
    }

    #[test]
    fn test_graph_edge_serialization() {
        let edge = GraphEdge {
            id: "e-12345678".to_string(),
            edge_type: EdgeType::DependsOn,
            from_node: "ra-a3f8.1".to_string(),
            to_node: "ra-a3f8.2".to_string(),
            label: Some("blocks".to_string()),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&edge).expect("Failed to serialize");
        let deserialized: GraphEdge = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(edge.id, deserialized.id);
        assert_eq!(edge.edge_type, deserialized.edge_type);
    }
}
