use anyhow::Result;
use async_trait::async_trait;
use rustagent::graph::store::{EdgeDirection, GraphStore, NodeQuery, WorkGraph};
use rustagent::graph::{EdgeType, GraphEdge, GraphNode, NodeStatus, NodeType};
use std::collections::HashMap;

/// Mock GraphStore for testing
pub struct MockGraphStore;

#[async_trait]
impl GraphStore for MockGraphStore {
    async fn create_node(&self, _node: &GraphNode) -> Result<()> {
        Ok(())
    }

    async fn update_node(
        &self,
        _id: &str,
        _status: Option<NodeStatus>,
        _title: Option<&str>,
        _description: Option<&str>,
        _metadata: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_node(&self, _id: &str) -> Result<Option<GraphNode>> {
        Ok(None)
    }

    async fn query_nodes(&self, _query: &NodeQuery) -> Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn claim_task(&self, _node_id: &str, _agent_id: &str) -> Result<bool> {
        Ok(false)
    }

    async fn get_ready_tasks(&self, _goal_id: &str) -> Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_next_task(&self, _goal_id: &str) -> Result<Option<GraphNode>> {
        Ok(None)
    }

    async fn add_edge(&self, _edge: &GraphEdge) -> Result<()> {
        Ok(())
    }

    async fn remove_edge(&self, _edge_id: &str) -> Result<()> {
        Ok(())
    }

    async fn get_edges(
        &self,
        _node_id: &str,
        _direction: EdgeDirection,
    ) -> Result<Vec<(GraphEdge, GraphNode)>> {
        Ok(vec![])
    }

    async fn get_children(&self, _node_id: &str) -> Result<Vec<(GraphNode, EdgeType)>> {
        Ok(vec![])
    }

    async fn get_subtree(&self, _node_id: &str) -> Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_active_decisions(&self, _project_id: &str) -> Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_full_graph(&self, _goal_id: &str) -> Result<WorkGraph> {
        Ok(WorkGraph {
            nodes: vec![],
            edges: vec![],
        })
    }

    async fn search_nodes(
        &self,
        _query: &str,
        _project_id: Option<&str>,
        _node_type: Option<NodeType>,
        _limit: usize,
    ) -> Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn next_child_seq(&self, _parent_id: &str) -> Result<u32> {
        Ok(1)
    }
}
