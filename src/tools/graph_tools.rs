use crate::graph::store::{GraphStore, NodeQuery};
use crate::graph::{
    EdgeType, GraphEdge, GraphNode, NodeStatus, NodeType, generate_child_id, generate_edge_id,
    generate_goal_id,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::Tool;

/// Low-level tool for creating nodes
pub struct CreateNodeTool {
    store: Arc<dyn GraphStore>,
}

impl CreateNodeTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for CreateNodeTool {
    fn name(&self) -> &str {
        "create_node"
    }

    fn description(&self) -> &str {
        "Create a new node in the work graph"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_type": {
                    "type": "string",
                    "enum": ["goal", "task", "decision", "option", "outcome", "observation", "revisit"],
                    "description": "Type of node to create"
                },
                "title": {
                    "type": "string",
                    "description": "Title of the node"
                },
                "description": {
                    "type": "string",
                    "description": "Description of the node"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID (required)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Parent node ID (optional, creates as child if provided)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["critical", "high", "medium", "low"],
                    "description": "Priority level (optional)"
                },
                "metadata": {
                    "type": "object",
                    "description": "Additional metadata (optional)"
                }
            },
            "required": ["node_type", "title", "description", "project_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let node_type_str = params["node_type"]
            .as_str()
            .context("Missing 'node_type' parameter")?;
        let node_type: NodeType = node_type_str.parse()?;

        let title = params["title"]
            .as_str()
            .context("Missing 'title' parameter")?
            .to_string();

        let description = params["description"]
            .as_str()
            .context("Missing 'description' parameter")?
            .to_string();

        let project_id = params["project_id"]
            .as_str()
            .context("Missing 'project_id' parameter")?
            .to_string();

        let parent_id_opt = params["parent_id"].as_str().map(|s| s.to_string());

        // Generate ID based on parent
        let id = if let Some(p_id) = &parent_id_opt {
            let seq = self.store.next_child_seq(p_id).await?;
            generate_child_id(p_id, seq)
        } else {
            generate_goal_id()
        };

        // Parse priority if provided
        let priority = params["priority"].as_str().and_then(|p| p.parse().ok());

        // Parse metadata if provided
        let metadata: HashMap<String, String> = params["metadata"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.to_string())).collect())
            .unwrap_or_default();

        // Create the node
        let node = GraphNode {
            id: id.clone(),
            project_id,
            node_type,
            title,
            description,
            status: NodeStatus::Pending,
            priority,
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata,
        };

        self.store.create_node(&node).await?;

        // If parent was provided, a Contains edge is created automatically in create_node
        Ok(json!({
            "id": id,
            "message": "Node created successfully"
        })
        .to_string())
    }
}

/// Low-level tool for updating nodes
pub struct UpdateNodeTool {
    store: Arc<dyn GraphStore>,
}

impl UpdateNodeTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for UpdateNodeTool {
    fn name(&self) -> &str {
        "update_node"
    }

    fn description(&self) -> &str {
        "Update a node in the work graph"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "ID of node to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "active", "completed", "cancelled", "ready", "claimed",
                             "in_progress", "review", "blocked", "failed", "decided", "superseded",
                             "abandoned", "chosen", "rejected"],
                    "description": "New status (optional)"
                },
                "title": {
                    "type": "string",
                    "description": "New title (optional)"
                },
                "description": {
                    "type": "string",
                    "description": "New description (optional)"
                },
                "metadata": {
                    "type": "object",
                    "description": "New metadata (optional)"
                }
            },
            "required": ["node_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let node_id = params["node_id"]
            .as_str()
            .context("Missing 'node_id' parameter")?;

        let status = params["status"].as_str().and_then(|s| s.parse().ok());

        let title = params["title"].as_str();
        let description = params["description"].as_str();

        let metadata: Option<HashMap<String, String>> = params["metadata"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.to_string())).collect());

        self.store
            .update_node(node_id, status, title, description, metadata.as_ref())
            .await?;

        Ok(json!({
            "message": "Node updated successfully"
        })
        .to_string())
    }
}

/// Low-level tool for adding edges
pub struct AddEdgeTool {
    store: Arc<dyn GraphStore>,
}

impl AddEdgeTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for AddEdgeTool {
    fn name(&self) -> &str {
        "add_edge"
    }

    fn description(&self) -> &str {
        "Add an edge between two nodes in the work graph"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edge_type": {
                    "type": "string",
                    "enum": ["contains", "depends_on", "leads_to", "chosen", "rejected", "supersedes", "informs"],
                    "description": "Type of edge"
                },
                "from_node": {
                    "type": "string",
                    "description": "Source node ID"
                },
                "to_node": {
                    "type": "string",
                    "description": "Target node ID"
                },
                "label": {
                    "type": "string",
                    "description": "Edge label (optional)"
                }
            },
            "required": ["edge_type", "from_node", "to_node"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let edge_type_str = params["edge_type"]
            .as_str()
            .context("Missing 'edge_type' parameter")?;
        let edge_type: EdgeType = edge_type_str.parse()?;

        let from_node = params["from_node"]
            .as_str()
            .context("Missing 'from_node' parameter")?
            .to_string();

        let to_node = params["to_node"]
            .as_str()
            .context("Missing 'to_node' parameter")?
            .to_string();

        let label = params["label"].as_str().map(|s| s.to_string());

        let edge = GraphEdge {
            id: generate_edge_id(),
            edge_type,
            from_node,
            to_node,
            label,
            created_at: Utc::now(),
        };

        self.store.add_edge(&edge).await?;

        Ok(json!({
            "id": edge.id,
            "message": "Edge created successfully"
        })
        .to_string())
    }
}

/// Low-level tool for querying nodes
pub struct QueryNodesTool {
    store: Arc<dyn GraphStore>,
}

impl QueryNodesTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for QueryNodesTool {
    fn name(&self) -> &str {
        "query_nodes"
    }

    fn description(&self) -> &str {
        "Query nodes in the work graph with flexible filters"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_type": {
                    "type": "string",
                    "enum": ["goal", "task", "decision", "option", "outcome", "observation", "revisit"],
                    "description": "Filter by node type (optional)"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "active", "completed", "cancelled", "ready", "claimed",
                             "in_progress", "review", "blocked", "failed", "decided", "superseded",
                             "abandoned", "chosen", "rejected"],
                    "description": "Filter by status (optional)"
                },
                "project_id": {
                    "type": "string",
                    "description": "Filter by project (optional)"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Filter by parent node (optional)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let query = NodeQuery {
            node_type: params["node_type"].as_str().and_then(|s| s.parse().ok()),
            status: params["status"].as_str().and_then(|s| s.parse().ok()),
            project_id: params["project_id"].as_str().map(|s| s.to_string()),
            parent_id: params["parent_id"].as_str().map(|s| s.to_string()),
            query: None,
        };

        let nodes = self.store.query_nodes(&query).await?;

        Ok(json!(nodes).to_string())
    }
}

/// Low-level tool for full-text search
pub struct SearchNodesTool {
    store: Arc<dyn GraphStore>,
}

impl SearchNodesTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for SearchNodesTool {
    fn name(&self) -> &str {
        "search_nodes"
    }

    fn description(&self) -> &str {
        "Full-text search for nodes in the work graph"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "project_id": {
                    "type": "string",
                    "description": "Filter by project (optional)"
                },
                "node_type": {
                    "type": "string",
                    "enum": ["goal", "task", "decision", "option", "outcome", "observation", "revisit"],
                    "description": "Filter by node type (optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 50)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let query = params["query"]
            .as_str()
            .context("Missing 'query' parameter")?;

        let project_id = params["project_id"].as_str();
        let node_type = params["node_type"].as_str().and_then(|s| s.parse().ok());
        let limit = params["limit"].as_u64().map(|n| n as usize).unwrap_or(50);

        let results = self
            .store
            .search_nodes(query, project_id, node_type, limit)
            .await?;

        Ok(json!(results).to_string())
    }
}

/// High-level tool for claiming a task
pub struct ClaimTaskTool {
    store: Arc<dyn GraphStore>,
}

impl ClaimTaskTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ClaimTaskTool {
    fn name(&self) -> &str {
        "claim_task"
    }

    fn description(&self) -> &str {
        "Atomically claim a task for execution"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "node_id": {
                    "type": "string",
                    "description": "Task node ID"
                },
                "agent_id": {
                    "type": "string",
                    "description": "Agent ID claiming the task"
                }
            },
            "required": ["node_id", "agent_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let node_id = params["node_id"]
            .as_str()
            .context("Missing 'node_id' parameter")?;

        let agent_id = params["agent_id"]
            .as_str()
            .context("Missing 'agent_id' parameter")?;

        let claimed = self.store.claim_task(node_id, agent_id).await?;

        Ok(json!({
            "claimed": claimed,
            "message": if claimed {
                "Task claimed successfully"
            } else {
                "Task was not in Ready state (may have been claimed by another agent)"
            }
        })
        .to_string())
    }
}

/// High-level tool for logging a decision with options
pub struct LogDecisionTool {
    store: Arc<dyn GraphStore>,
}

impl LogDecisionTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for LogDecisionTool {
    fn name(&self) -> &str {
        "log_decision"
    }

    fn description(&self) -> &str {
        "Log a decision point with multiple options"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Decision title"
                },
                "description": {
                    "type": "string",
                    "description": "Decision description"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID"
                },
                "parent_id": {
                    "type": "string",
                    "description": "Parent node ID (optional)"
                },
                "options": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "pros": { "type": "string" },
                            "cons": { "type": "string" }
                        },
                        "required": ["title", "description"]
                    },
                    "description": "Options for this decision"
                }
            },
            "required": ["title", "description", "project_id", "options"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let title = params["title"]
            .as_str()
            .context("Missing 'title' parameter")?
            .to_string();

        let description = params["description"]
            .as_str()
            .context("Missing 'description' parameter")?
            .to_string();

        let project_id = params["project_id"]
            .as_str()
            .context("Missing 'project_id' parameter")?
            .to_string();

        let parent_id_opt = params["parent_id"].as_str().map(|s| s.to_string());

        let options_arr = params["options"]
            .as_array()
            .context("Missing 'options' parameter")?;

        // Create the decision node
        let decision_id = if let Some(p_id) = &parent_id_opt {
            let seq = self.store.next_child_seq(p_id).await?;
            generate_child_id(p_id, seq)
        } else {
            generate_goal_id()
        };

        let decision_node = GraphNode {
            id: decision_id.clone(),
            project_id: project_id.clone(),
            node_type: NodeType::Decision,
            title,
            description,
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

        self.store.create_node(&decision_node).await?;

        // Create option nodes and LeadsTo edges
        let mut option_ids = vec![];
        for option in options_arr {
            let opt_title = option["title"]
                .as_str()
                .context("Missing option title")?
                .to_string();

            let opt_desc = option["description"]
                .as_str()
                .context("Missing option description")?
                .to_string();

            let seq = self.store.next_child_seq(&decision_id).await?;
            let option_id = generate_child_id(&decision_id, seq);

            let mut opt_metadata = HashMap::new();
            if let Some(pros) = option["pros"].as_str() {
                opt_metadata.insert("pros".to_string(), pros.to_string());
            }
            if let Some(cons) = option["cons"].as_str() {
                opt_metadata.insert("cons".to_string(), cons.to_string());
            }

            let option_node = GraphNode {
                id: option_id.clone(),
                project_id: project_id.clone(),
                node_type: NodeType::Option,
                title: opt_title,
                description: opt_desc,
                status: NodeStatus::Active,
                priority: None,
                assigned_to: None,
                created_by: None,
                labels: vec![],
                created_at: Utc::now(),
                started_at: None,
                completed_at: None,
                blocked_reason: None,
                metadata: opt_metadata,
            };

            self.store.create_node(&option_node).await?;

            // Create LeadsTo edge from decision to option
            let edge = GraphEdge {
                id: generate_edge_id(),
                edge_type: EdgeType::LeadsTo,
                from_node: decision_id.clone(),
                to_node: option_id.clone(),
                label: None,
                created_at: Utc::now(),
            };

            self.store.add_edge(&edge).await?;
            option_ids.push(option_id);
        }

        Ok(json!({
            "decision_id": decision_id,
            "option_ids": option_ids,
            "message": "Decision and options created successfully"
        })
        .to_string())
    }
}

/// High-level tool for choosing an option
pub struct ChooseOptionTool {
    store: Arc<dyn GraphStore>,
}

impl ChooseOptionTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for ChooseOptionTool {
    fn name(&self) -> &str {
        "choose_option"
    }

    fn description(&self) -> &str {
        "Choose an option for a decision"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "decision_id": {
                    "type": "string",
                    "description": "Decision node ID"
                },
                "option_id": {
                    "type": "string",
                    "description": "Option node ID to choose"
                },
                "rationale": {
                    "type": "string",
                    "description": "Rationale for the choice"
                }
            },
            "required": ["decision_id", "option_id", "rationale"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let decision_id = params["decision_id"]
            .as_str()
            .context("Missing 'decision_id' parameter")?;

        let option_id = params["option_id"]
            .as_str()
            .context("Missing 'option_id' parameter")?;

        let rationale = params["rationale"]
            .as_str()
            .context("Missing 'rationale' parameter")?;

        // Add Chosen edge from decision to chosen option
        let chosen_edge = GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::Chosen,
            from_node: decision_id.to_string(),
            to_node: option_id.to_string(),
            label: Some(rationale.to_string()),
            created_at: Utc::now(),
        };

        self.store.add_edge(&chosen_edge).await?;

        // Update chosen option status to Chosen
        self.store
            .update_node(option_id, Some(NodeStatus::Chosen), None, None, None)
            .await?;

        // Find other options and add Rejected edges
        // Query all options under this decision
        let query = NodeQuery {
            node_type: Some(NodeType::Option),
            status: None,
            project_id: None,
            parent_id: Some(decision_id.to_string()),
            query: None,
        };

        let options = self.store.query_nodes(&query).await?;
        for option in options {
            if option.id != option_id {
                // Add Rejected edge from decision to this option
                let rejected_edge = GraphEdge {
                    id: generate_edge_id(),
                    edge_type: EdgeType::Rejected,
                    from_node: decision_id.to_string(),
                    to_node: option.id.clone(),
                    label: None,
                    created_at: Utc::now(),
                };

                self.store.add_edge(&rejected_edge).await?;

                // Update option status to Rejected
                self.store
                    .update_node(&option.id, Some(NodeStatus::Rejected), None, None, None)
                    .await?;
            }
        }

        // Update decision status to Decided
        self.store
            .update_node(decision_id, Some(NodeStatus::Decided), None, None, None)
            .await?;

        Ok(json!({
            "message": "Option chosen successfully"
        })
        .to_string())
    }
}

/// High-level tool for recording an outcome
pub struct RecordOutcomeTool {
    store: Arc<dyn GraphStore>,
}

impl RecordOutcomeTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for RecordOutcomeTool {
    fn name(&self) -> &str {
        "record_outcome"
    }

    fn description(&self) -> &str {
        "Record the outcome of a task or decision"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "parent_id": {
                    "type": "string",
                    "description": "Parent node ID (task or decision)"
                },
                "title": {
                    "type": "string",
                    "description": "Outcome title"
                },
                "description": {
                    "type": "string",
                    "description": "Outcome description"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID"
                },
                "success": {
                    "type": "boolean",
                    "description": "Whether the outcome was successful"
                }
            },
            "required": ["parent_id", "title", "description", "project_id", "success"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let parent_id = params["parent_id"]
            .as_str()
            .context("Missing 'parent_id' parameter")?;

        let title = params["title"]
            .as_str()
            .context("Missing 'title' parameter")?
            .to_string();

        let description = params["description"]
            .as_str()
            .context("Missing 'description' parameter")?
            .to_string();

        let project_id = params["project_id"]
            .as_str()
            .context("Missing 'project_id' parameter")?
            .to_string();

        let success = params["success"]
            .as_bool()
            .context("Missing 'success' parameter")?;

        // Create outcome node
        let seq = self.store.next_child_seq(parent_id).await?;
        let outcome_id = generate_child_id(parent_id, seq);

        let mut metadata = HashMap::new();
        metadata.insert("success".to_string(), success.to_string());

        let outcome_node = GraphNode {
            id: outcome_id.clone(),
            project_id,
            node_type: NodeType::Outcome,
            title,
            description,
            status: NodeStatus::Completed,
            priority: None,
            assigned_to: None,
            created_by: None,
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: Some(Utc::now()),
            blocked_reason: None,
            metadata,
        };

        self.store.create_node(&outcome_node).await?;

        // Create LeadsTo edge from parent to outcome
        let edge = GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::LeadsTo,
            from_node: parent_id.to_string(),
            to_node: outcome_id.clone(),
            label: None,
            created_at: Utc::now(),
        };

        self.store.add_edge(&edge).await?;

        Ok(json!({
            "outcome_id": outcome_id,
            "message": "Outcome recorded successfully"
        })
        .to_string())
    }
}

/// High-level tool for recording an observation
pub struct RecordObservationTool {
    store: Arc<dyn GraphStore>,
}

impl RecordObservationTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for RecordObservationTool {
    fn name(&self) -> &str {
        "record_observation"
    }

    fn description(&self) -> &str {
        "Record an observation about the work"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Observation title"
                },
                "description": {
                    "type": "string",
                    "description": "Observation description"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID"
                },
                "related_node_id": {
                    "type": "string",
                    "description": "Related node ID for Informs edge (optional)"
                }
            },
            "required": ["title", "description", "project_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let title = params["title"]
            .as_str()
            .context("Missing 'title' parameter")?
            .to_string();

        let description = params["description"]
            .as_str()
            .context("Missing 'description' parameter")?
            .to_string();

        let project_id = params["project_id"]
            .as_str()
            .context("Missing 'project_id' parameter")?
            .to_string();

        let related_node_id = params["related_node_id"].as_str().map(|s| s.to_string());

        // Create observation node
        let observation_id = generate_goal_id();

        let observation_node = GraphNode {
            id: observation_id.clone(),
            project_id,
            node_type: NodeType::Observation,
            title,
            description,
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

        self.store.create_node(&observation_node).await?;

        // Create Informs edge if related node provided
        if let Some(related_id) = related_node_id {
            let edge = GraphEdge {
                id: generate_edge_id(),
                edge_type: EdgeType::Informs,
                from_node: observation_id.clone(),
                to_node: related_id,
                label: None,
                created_at: Utc::now(),
            };

            self.store.add_edge(&edge).await?;
        }

        Ok(json!({
            "observation_id": observation_id,
            "message": "Observation recorded successfully"
        })
        .to_string())
    }
}

/// High-level tool for revisiting a decision
pub struct RevisitTool {
    store: Arc<dyn GraphStore>,
}

impl RevisitTool {
    pub fn new(store: Arc<dyn GraphStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Tool for RevisitTool {
    fn name(&self) -> &str {
        "revisit"
    }

    fn description(&self) -> &str {
        "Revisit and potentially revise a past decision based on an outcome"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "outcome_id": {
                    "type": "string",
                    "description": "Outcome node ID"
                },
                "project_id": {
                    "type": "string",
                    "description": "Project ID"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for revisiting"
                },
                "new_decision_title": {
                    "type": "string",
                    "description": "Title for new decision if creating one (optional)"
                }
            },
            "required": ["outcome_id", "project_id", "reason"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let outcome_id = params["outcome_id"]
            .as_str()
            .context("Missing 'outcome_id' parameter")?;

        let project_id = params["project_id"]
            .as_str()
            .context("Missing 'project_id' parameter")?
            .to_string();

        let reason = params["reason"]
            .as_str()
            .context("Missing 'reason' parameter")?
            .to_string();

        let new_decision_title = params["new_decision_title"].as_str();

        // Create revisit node
        let revisit_id = generate_goal_id();

        let revisit_node = GraphNode {
            id: revisit_id.clone(),
            project_id: project_id.clone(),
            node_type: NodeType::Revisit,
            title: format!("Revisit of {}", outcome_id),
            description: reason,
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

        self.store.create_node(&revisit_node).await?;

        // Create LeadsTo edge from outcome to revisit
        let edge = GraphEdge {
            id: generate_edge_id(),
            edge_type: EdgeType::LeadsTo,
            from_node: outcome_id.to_string(),
            to_node: revisit_id.clone(),
            label: None,
            created_at: Utc::now(),
        };

        self.store.add_edge(&edge).await?;

        let mut result = json!({
            "revisit_id": revisit_id,
            "message": "Revisit recorded successfully"
        });

        // Create new decision if title provided
        if let Some(title) = new_decision_title {
            let decision_id = generate_goal_id();

            let decision_node = GraphNode {
                id: decision_id.clone(),
                project_id,
                node_type: NodeType::Decision,
                title: title.to_string(),
                description: "Decision created from revisit".to_string(),
                status: NodeStatus::Pending,
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

            self.store.create_node(&decision_node).await?;

            // Create LeadsTo edge from revisit to new decision
            let decision_edge = GraphEdge {
                id: generate_edge_id(),
                edge_type: EdgeType::LeadsTo,
                from_node: revisit_id,
                to_node: decision_id.clone(),
                label: None,
                created_at: Utc::now(),
            };

            self.store.add_edge(&decision_edge).await?;

            if let Some(obj) = result.as_object_mut() {
                obj.insert("decision_id".to_string(), json!(decision_id));
            }
        }

        Ok(result.to_string())
    }
}
