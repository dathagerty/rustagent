use crate::graph::{GraphEdge, GraphNode, NodeStatus, NodeType};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Direction for edge queries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// Edges going out from a node (from_node = id)
    Outgoing,
    /// Edges coming in to a node (to_node = id)
    Incoming,
    /// Both directions
    Both,
}

/// A graph containing nodes and edges (used in history/full graph queries)
#[derive(Debug, Clone)]
pub struct WorkGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Query builder for flexible node searches
#[derive(Debug, Clone)]
pub struct NodeQuery {
    pub node_type: Option<NodeType>,
    pub status: Option<NodeStatus>,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub query: Option<String>,
}

/// The GraphStore trait defines all operations on the work graph.
/// Implementations use BEGIN IMMEDIATE transactions for atomic writes.
#[async_trait]
pub trait GraphStore: Send + Sync {
    // ===== Node CRUD =====

    /// Create a new node in the store
    async fn create_node(&self, node: &GraphNode) -> Result<()>;

    /// Update node fields. Only provided fields are updated.
    async fn update_node(
        &self,
        id: &str,
        status: Option<NodeStatus>,
        title: Option<&str>,
        description: Option<&str>,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<()>;

    /// Retrieve a node by ID
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>>;

    /// Query nodes with optional filters
    async fn query_nodes(&self, query: &NodeQuery) -> Result<Vec<GraphNode>>;

    // ===== Task-specific operations =====

    /// Atomically claim a task (status Ready -> Claimed).
    /// Returns true if claimed, false if task was not in Ready state (another worker got it first).
    async fn claim_task(&self, node_id: &str, agent_id: &str) -> Result<bool>;

    /// Get all ready tasks under a goal
    async fn get_ready_tasks(&self, goal_id: &str) -> Result<Vec<GraphNode>>;

    /// Get the next recommended task: highest priority, break ties by downstream unblock count
    async fn get_next_task(&self, goal_id: &str) -> Result<Option<GraphNode>>;

    // ===== Edge operations =====

    /// Add an edge connecting two nodes
    async fn add_edge(&self, edge: &GraphEdge) -> Result<()>;

    /// Remove an edge by ID
    async fn remove_edge(&self, edge_id: &str) -> Result<()>;

    /// Get edges involving a node in the specified direction, with the related nodes
    async fn get_edges(
        &self,
        node_id: &str,
        direction: EdgeDirection,
    ) -> Result<Vec<(GraphEdge, GraphNode)>>;

    // ===== Graph queries =====

    /// Get immediate children of a node via Contains edges
    async fn get_children(&self, node_id: &str) -> Result<Vec<(GraphNode, crate::graph::EdgeType)>>;

    /// Get all descendants recursively via Contains edges
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<GraphNode>>;

    /// Get active decisions for a project (decision_active or option_active, no abandoned/superseded)
    async fn get_active_decisions(&self, project_id: &str) -> Result<Vec<GraphNode>>;

    /// Get the full graph: goal + all descendants + all edges among them
    async fn get_full_graph(&self, goal_id: &str) -> Result<WorkGraph>;

    /// Full-text search nodes by query, optionally filtered by project and node_type
    async fn search_nodes(
        &self,
        query: &str,
        project_id: Option<&str>,
        node_type: Option<NodeType>,
        limit: usize,
    ) -> Result<Vec<GraphNode>>;

    // ===== Utility =====

    /// Get the next child sequence number for a parent, atomically increment, and return old value
    async fn next_child_seq(&self, parent_id: &str) -> Result<u32>;
}
