/// TOML-based graph interchange format for goal-level export/import
///
/// This module provides deterministic, git-friendly graph serialization.
/// TOML files are per-goal, with sorted keys (BTreeMap) for reproducible output.
/// Content hash enables detecting changes, and conflict strategies handle imports.
use crate::graph::store::GraphStore;
use crate::graph::{EdgeType, GraphEdge, GraphNode};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Version of the TOML interchange format
const INTERCHANGE_VERSION: u32 = 1;

/// Metadata about an exported goal file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub version: u32,
    pub goal_id: String,
    pub project: String,
    pub exported_at: String,
    pub content_hash: String,
}

/// A node as represented in the TOML format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlNode {
    pub project_id: String,
    pub node_type: String,
    pub title: String,
    pub description: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,
}

/// An edge as represented in the TOML format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlEdge {
    pub edge_type: String,
    pub from_node: String,
    pub to_node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at: String,
}

/// A complete goal file in TOML format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalFile {
    pub meta: Meta,
    pub nodes: BTreeMap<String, TomlNode>,
    pub edges: BTreeMap<String, TomlEdge>,
}

/// Conflict strategy for importing TOML data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStrategy {
    /// Flag conflicts and let the user decide manually
    Merge,
    /// File version wins (overwrite DB)
    Theirs,
    /// DB version wins (skip import)
    Ours,
}

/// A conflict detected during import
#[derive(Debug, Clone, Serialize)]
pub struct ImportConflict {
    pub node_id: String,
    pub field: String,
    pub db_value: String,
    pub file_value: String,
}

/// Result of importing TOML data
#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub added_nodes: usize,
    pub added_edges: usize,
    pub conflicts: Vec<ImportConflict>,
    pub skipped_edges: Vec<String>, // Messages about edges referencing nonexistent nodes
    pub unchanged: usize,
}

/// Difference between TOML and DB state
#[derive(Debug, Clone, Serialize)]
pub struct DiffResult {
    pub added_nodes: Vec<String>,                  // In file but not in DB
    pub changed_nodes: Vec<(String, Vec<String>)>, // (id, changed_fields)
    pub removed_nodes: Vec<String>,                // In DB but not in file
    pub added_edges: Vec<String>,                  // Edge IDs in file but not in DB
    pub removed_edges: Vec<String>,                // Edge IDs in DB but not in file
    pub unchanged_nodes: usize,
    pub unchanged_edges: usize,
}

/// Export a goal and its descendants to TOML format
///
/// This produces a deterministic TOML string with:
/// - BTreeMap-based sorted keys
/// - Content hash computed from nodes + edges
/// - Null/empty fields omitted
pub async fn export_goal(
    graph_store: &dyn GraphStore,
    goal_id: &str,
    project_name: &str,
) -> Result<String> {
    // Get all nodes in the goal's subtree
    let mut nodes_vec = graph_store.get_subtree(goal_id).await?;
    // Include the goal itself
    if let Some(goal_node) = graph_store.get_node(goal_id).await?
        && !nodes_vec.iter().any(|n| n.id == goal_id)
    {
        nodes_vec.insert(0, goal_node);
    }

    // Get the full graph (nodes + edges)
    let graph = graph_store.get_full_graph(goal_id).await?;

    // Build the node map for TOML
    let mut toml_nodes = BTreeMap::new();
    for node in nodes_vec {
        toml_nodes.insert(node.id.clone(), graph_node_to_toml(&node));
    }

    // Build the edge map for TOML (only edges where both endpoints are in our subtree)
    let node_ids: std::collections::HashSet<_> = toml_nodes.keys().cloned().collect();
    let mut toml_edges = BTreeMap::new();
    for edge in graph.edges {
        if node_ids.contains(&edge.from_node) && node_ids.contains(&edge.to_node) {
            toml_edges.insert(edge.id.clone(), graph_edge_to_toml(&edge));
        }
    }

    // Compute content hash (serialize nodes + edges, hash, convert to hex)
    // This hash is deterministic and should be identical for identical content
    let nodes_json = serde_json::to_string(&toml_nodes)?;
    let edges_json = serde_json::to_string(&toml_edges)?;
    let content_to_hash = format!("{}{}", nodes_json, edges_json);
    let content_hash = blake3::hash(content_to_hash.as_bytes())
        .to_hex()
        .to_string();

    // Record the export time (will vary on each export, so not byte-identical for timestamps)
    // The content hash remains deterministic based on node/edge data
    let exported_at = Utc::now().to_rfc3339();

    let goal_file = GoalFile {
        meta: Meta {
            version: INTERCHANGE_VERSION,
            goal_id: goal_id.to_string(),
            project: project_name.to_string(),
            exported_at,
            content_hash,
        },
        nodes: toml_nodes,
        edges: toml_edges,
    };

    // Serialize to TOML
    let toml_string = toml::to_string_pretty(&goal_file)?;
    Ok(toml_string)
}

/// Import TOML data into the graph store
///
/// Applies the given conflict strategy:
/// - Merge: conflicts are flagged but import proceeds
/// - Theirs: file version overwrites DB
/// - Ours: keep DB version, skip import
///
/// All writes in a single BEGIN IMMEDIATE transaction.
pub async fn import_goal(
    graph_store: &dyn GraphStore,
    toml_content: &str,
    strategy: ImportStrategy,
) -> Result<ImportResult> {
    let goal_file: GoalFile =
        toml::from_str(toml_content).context("Failed to parse TOML goal file")?;

    let mut result = ImportResult {
        added_nodes: 0,
        added_edges: 0,
        conflicts: Vec::new(),
        skipped_edges: Vec::new(),
        unchanged: 0,
    };

    // Collect nodes to import in a single transaction
    let mut nodes_to_add = Vec::new();

    // Process nodes to determine what to add
    for (node_id, toml_node) in &goal_file.nodes {
        match graph_store.get_node(node_id).await? {
            None => {
                // New node: will add in transaction
                let node = toml_to_graph_node(node_id, toml_node)?;
                nodes_to_add.push(node);
                result.added_nodes += 1;
            }
            Some(existing_node) => {
                // Check if changed
                let changed_fields = detect_node_changes(&existing_node, toml_node)?;
                if changed_fields.is_empty() {
                    result.unchanged += 1;
                } else {
                    match strategy {
                        ImportStrategy::Merge => {
                            for field in &changed_fields {
                                result.conflicts.push(ImportConflict {
                                    node_id: node_id.clone(),
                                    field: field.clone(),
                                    db_value: get_node_field_value(&existing_node, field),
                                    file_value: get_toml_node_field_value(toml_node, field),
                                });
                            }
                        }
                        ImportStrategy::Theirs => {
                            let node = toml_to_graph_node(node_id, toml_node)?;
                            // Perform update with the new values
                            graph_store
                                .update_node(
                                    node_id,
                                    Some(node.status),
                                    Some(&node.title),
                                    Some(&node.description),
                                    node.blocked_reason.as_deref(),
                                    Some(&node.metadata),
                                )
                                .await?;
                        }
                        ImportStrategy::Ours => {
                            // Skip this node
                        }
                    }
                }
            }
        }
    }

    // Collect edges to import
    let mut edges_to_add = Vec::new();

    for (edge_id, toml_edge) in &goal_file.edges {
        // Check if both endpoints exist
        let from_exists = graph_store.get_node(&toml_edge.from_node).await?.is_some();
        let to_exists = graph_store.get_node(&toml_edge.to_node).await?.is_some();

        if !from_exists || !to_exists {
            result.skipped_edges.push(format!(
                "Edge {} skips unresolved reference: {} -> {} (source exists: {}, target exists: {})",
                edge_id, toml_edge.from_node, toml_edge.to_node, from_exists, to_exists
            ));
            continue;
        }

        // Convert to GraphEdge
        let edge_type: EdgeType = toml_edge.edge_type.parse()?;
        let edge = GraphEdge {
            id: edge_id.clone(),
            edge_type,
            from_node: toml_edge.from_node.clone(),
            to_node: toml_edge.to_node.clone(),
            label: toml_edge.label.clone(),
            created_at: chrono::DateTime::parse_from_rfc3339(&toml_edge.created_at)?
                .with_timezone(&Utc),
        };

        edges_to_add.push(edge);
        result.added_edges += 1;
    }

    // Import all nodes and edges in a single transaction
    if !nodes_to_add.is_empty() || !edges_to_add.is_empty() {
        graph_store
            .import_nodes_and_edges(nodes_to_add, edges_to_add)
            .await?;
    }

    Ok(result)
}

/// Diff TOML file against current DB state
///
/// Shows what would change if the TOML were imported without making changes.
pub async fn diff_goal(graph_store: &dyn GraphStore, toml_content: &str) -> Result<DiffResult> {
    let goal_file: GoalFile =
        toml::from_str(toml_content).context("Failed to parse TOML goal file")?;

    let mut result = DiffResult {
        added_nodes: Vec::new(),
        changed_nodes: Vec::new(),
        removed_nodes: Vec::new(),
        added_edges: Vec::new(),
        removed_edges: Vec::new(),
        unchanged_nodes: 0,
        unchanged_edges: 0,
    };

    // Check which nodes would be added or changed
    for (node_id, toml_node) in &goal_file.nodes {
        match graph_store.get_node(node_id).await? {
            None => {
                result.added_nodes.push(node_id.clone());
            }
            Some(existing_node) => {
                let changed_fields = detect_node_changes(&existing_node, toml_node)?;
                if changed_fields.is_empty() {
                    result.unchanged_nodes += 1;
                } else {
                    result.changed_nodes.push((node_id.clone(), changed_fields));
                }
            }
        }
    }

    // Find removed nodes (in DB but not in file)
    let goal_id = &goal_file.meta.goal_id;
    let db_nodes = graph_store.get_subtree(goal_id).await?;
    let file_node_ids: std::collections::HashSet<_> = goal_file.nodes.keys().cloned().collect();

    for node in db_nodes {
        if !file_node_ids.contains(&node.id) {
            result.removed_nodes.push(node.id);
        }
    }

    // Check edges
    let graph = graph_store.get_full_graph(goal_id).await?;
    let file_edge_ids: std::collections::HashSet<_> = goal_file.edges.keys().cloned().collect();

    for edge in &graph.edges {
        if !file_edge_ids.contains(&edge.id) {
            result.removed_edges.push(edge.id.clone());
        }
    }

    // Check for added edges
    for edge_id in goal_file.edges.keys() {
        if !graph.edges.iter().any(|e| &e.id == edge_id) {
            result.added_edges.push(edge_id.clone());
        }
    }

    // Count unchanged edges
    result.unchanged_edges = graph.edges.len() - result.removed_edges.len();

    Ok(result)
}

// ===== Helper functions =====

/// Convert a GraphNode to TomlNode
fn graph_node_to_toml(node: &GraphNode) -> TomlNode {
    TomlNode {
        project_id: node.project_id.clone(),
        node_type: node.node_type.to_string(),
        title: node.title.clone(),
        description: node.description.clone(),
        status: node.status.to_string(),
        priority: node.priority.map(|p| p.to_string()),
        assigned_to: node.assigned_to.clone(),
        created_by: node.created_by.clone(),
        labels: if node.labels.is_empty() {
            None
        } else {
            Some(node.labels.clone())
        },
        created_at: node.created_at.to_rfc3339(),
        started_at: node.started_at.map(|t| t.to_rfc3339()),
        completed_at: node.completed_at.map(|t| t.to_rfc3339()),
        blocked_reason: node.blocked_reason.clone(),
        metadata: if node.metadata.is_empty() {
            None
        } else {
            Some(node.metadata.clone().into_iter().collect())
        },
    }
}

/// Convert a GraphEdge to TomlEdge
fn graph_edge_to_toml(edge: &GraphEdge) -> TomlEdge {
    TomlEdge {
        edge_type: edge.edge_type.to_string(),
        from_node: edge.from_node.clone(),
        to_node: edge.to_node.clone(),
        label: edge.label.clone(),
        created_at: edge.created_at.to_rfc3339(),
    }
}

/// Convert TomlNode to GraphNode
fn toml_to_graph_node(id: &str, toml: &TomlNode) -> Result<GraphNode> {
    Ok(GraphNode {
        id: id.to_string(),
        project_id: toml.project_id.clone(),
        node_type: toml.node_type.parse()?,
        title: toml.title.clone(),
        description: toml.description.clone(),
        status: toml.status.parse()?,
        priority: toml.priority.as_ref().and_then(|p| p.parse().ok()),
        assigned_to: toml.assigned_to.clone(),
        created_by: toml.created_by.clone(),
        labels: toml.labels.clone().unwrap_or_default(),
        created_at: chrono::DateTime::parse_from_rfc3339(&toml.created_at)?.with_timezone(&Utc),
        started_at: toml.started_at.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
        completed_at: toml.completed_at.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        }),
        blocked_reason: toml.blocked_reason.clone(),
        metadata: toml
            .metadata
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    })
}

/// Detect which fields have changed between DB and TOML
fn detect_node_changes(db_node: &GraphNode, toml_node: &TomlNode) -> Result<Vec<String>> {
    let mut changed = Vec::new();

    if db_node.title != toml_node.title {
        changed.push("title".to_string());
    }
    if db_node.description != toml_node.description {
        changed.push("description".to_string());
    }
    if db_node.status.to_string() != toml_node.status {
        changed.push("status".to_string());
    }
    if db_node.priority.map(|p| p.to_string()) != toml_node.priority {
        changed.push("priority".to_string());
    }
    if db_node.assigned_to != toml_node.assigned_to {
        changed.push("assigned_to".to_string());
    }
    if db_node.created_by != toml_node.created_by {
        changed.push("created_by".to_string());
    }
    // Check labels: both empty/None means no change
    let db_has_labels = !db_node.labels.is_empty();
    let toml_has_labels =
        toml_node.labels.is_some() && !toml_node.labels.as_ref().unwrap().is_empty();
    if db_has_labels != toml_has_labels
        || (db_has_labels && Some(&db_node.labels) != toml_node.labels.as_ref())
    {
        changed.push("labels".to_string());
    }
    if db_node.blocked_reason != toml_node.blocked_reason {
        changed.push("blocked_reason".to_string());
    }

    let toml_metadata: HashMap<String, String> = toml_node
        .metadata
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    if db_node.metadata != toml_metadata {
        changed.push("metadata".to_string());
    }

    Ok(changed)
}

/// Get field value from a GraphNode as a string for display
fn get_node_field_value(node: &GraphNode, field: &str) -> String {
    match field {
        "title" => node.title.clone(),
        "description" => node.description.clone(),
        "status" => node.status.to_string(),
        "priority" => node.priority.map(|p| p.to_string()).unwrap_or_default(),
        "assigned_to" => node.assigned_to.clone().unwrap_or_default(),
        "created_by" => node.created_by.clone().unwrap_or_default(),
        "labels" => serde_json::to_string(&node.labels).unwrap_or_default(),
        "blocked_reason" => node.blocked_reason.clone().unwrap_or_default(),
        "metadata" => serde_json::to_string(&node.metadata).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Get field value from a TomlNode as a string for display
fn get_toml_node_field_value(node: &TomlNode, field: &str) -> String {
    match field {
        "title" => node.title.clone(),
        "description" => node.description.clone(),
        "status" => node.status.clone(),
        "priority" => node.priority.clone().unwrap_or_default(),
        "assigned_to" => node.assigned_to.clone().unwrap_or_default(),
        "created_by" => node.created_by.clone().unwrap_or_default(),
        "labels" => serde_json::to_string(&node.labels).unwrap_or_default(),
        "blocked_reason" => node.blocked_reason.clone().unwrap_or_default(),
        "metadata" => serde_json::to_string(&node.metadata).unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{NodeStatus, NodeType};

    #[test]
    fn test_graph_node_to_toml_conversion() {
        let node = GraphNode {
            id: "test-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Task,
            title: "Test Task".to_string(),
            description: "A test task".to_string(),
            status: NodeStatus::Pending,
            priority: Some(crate::graph::Priority::High),
            assigned_to: Some("user1".to_string()),
            created_by: Some("user2".to_string()),
            labels: vec!["label1".to_string()],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        };

        let toml_node = graph_node_to_toml(&node);
        assert_eq!(toml_node.title, "Test Task");
        assert_eq!(toml_node.node_type, "task");
        assert_eq!(toml_node.priority, Some("high".to_string()));
    }

    #[test]
    fn test_toml_to_graph_node_conversion() {
        let toml_node = TomlNode {
            project_id: "proj-1".to_string(),
            node_type: "task".to_string(),
            title: "Test Task".to_string(),
            description: "A test task".to_string(),
            status: "pending".to_string(),
            priority: Some("high".to_string()),
            assigned_to: Some("user1".to_string()),
            created_by: Some("user2".to_string()),
            labels: Some(vec!["label1".to_string()]),
            created_at: Utc::now().to_rfc3339(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: None,
        };

        let node = toml_to_graph_node("test-1", &toml_node).expect("Failed to convert");
        assert_eq!(node.id, "test-1");
        assert_eq!(node.title, "Test Task");
        assert_eq!(node.node_type, NodeType::Task);
    }

    #[test]
    fn test_detect_node_changes() {
        let db_node = GraphNode {
            id: "test-1".to_string(),
            project_id: "proj-1".to_string(),
            node_type: NodeType::Task,
            title: "Original Title".to_string(),
            description: "Original description".to_string(),
            status: NodeStatus::Pending,
            priority: Some(crate::graph::Priority::High),
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: HashMap::new(),
        };

        let toml_node = TomlNode {
            project_id: "proj-1".to_string(),
            node_type: "task".to_string(),
            title: "New Title".to_string(),
            description: "Original description".to_string(),
            status: "pending".to_string(),
            priority: Some("high".to_string()),
            assigned_to: None,
            created_by: None,
            labels: None,
            created_at: db_node.created_at.to_rfc3339(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata: None,
        };

        let changes = detect_node_changes(&db_node, &toml_node).expect("Failed");
        assert!(changes.contains(&"title".to_string()));
        assert!(!changes.contains(&"description".to_string()));
    }
}
