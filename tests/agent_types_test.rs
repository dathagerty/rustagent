use async_trait::async_trait;
use rustagent::agent::profile::AgentProfile;
use rustagent::agent::{Agent, AgentContext, AgentId, AgentOutcome};
use rustagent::graph::{EdgeType, GraphNode, NodeStatus};
use rustagent::security::SecurityScope;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Mock agent for testing trait implementation
struct MockAgent {
    id: AgentId,
    profile: AgentProfile,
}

#[async_trait]
impl Agent for MockAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn profile(&self) -> &AgentProfile {
        &self.profile
    }

    async fn run(&self, _ctx: AgentContext) -> anyhow::Result<AgentOutcome> {
        Ok(AgentOutcome::Completed {
            summary: "mock completed".to_string(),
        })
    }

    fn cancel(&self) {
        // No-op stub for Phase 1d
    }
}

#[test]
fn test_agent_trait_compiles() {
    // P1d.AC2.1: Verify Agent trait can be implemented
    let profile = AgentProfile {
        name: "test".to_string(),
        extends: None,
        role: "test role".to_string(),
        system_prompt: "test prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let _agent = MockAgent {
        id: "agent-1".to_string(),
        profile,
    };
    // If this compiles, the trait is correctly defined
}

#[test]
fn test_agent_context_construction() {
    // P1d.AC2.2: Verify AgentContext can be constructed with all fields
    let profile = AgentProfile {
        name: "test".to_string(),
        extends: None,
        role: "test role".to_string(),
        system_prompt: "test prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let _ctx = AgentContext {
        work_package_tasks: vec![],
        relevant_decisions: vec![],
        handoff_notes: Some("test notes".to_string()),
        agents_md_summaries: vec![("path".to_string(), "summary".to_string())],
        profile,
        project_path: PathBuf::from("/tmp"),
        graph_store: Arc::new(MockGraphStore),
    };
    // If this compiles, the struct is correctly defined
}

#[test]
fn test_agent_outcome_completed() {
    // P1d.AC2.3: Verify Completed variant
    let outcome = AgentOutcome::Completed {
        summary: "task completed".to_string(),
    };

    match outcome {
        AgentOutcome::Completed { summary } => {
            assert_eq!(summary, "task completed");
        }
        _ => panic!("Expected Completed variant"),
    }
}

#[test]
fn test_agent_outcome_blocked() {
    // P1d.AC2.3: Verify Blocked variant
    let outcome = AgentOutcome::Blocked {
        reason: "blocked by dependency".to_string(),
    };

    match outcome {
        AgentOutcome::Blocked { reason } => {
            assert_eq!(reason, "blocked by dependency");
        }
        _ => panic!("Expected Blocked variant"),
    }
}

#[test]
fn test_agent_outcome_failed() {
    // P1d.AC2.3: Verify Failed variant
    let outcome = AgentOutcome::Failed {
        error: "something went wrong".to_string(),
    };

    match outcome {
        AgentOutcome::Failed { error } => {
            assert_eq!(error, "something went wrong");
        }
        _ => panic!("Expected Failed variant"),
    }
}

#[test]
fn test_agent_outcome_token_budget_exhausted() {
    // P1d.AC2.3: Verify TokenBudgetExhausted variant
    let outcome = AgentOutcome::TokenBudgetExhausted {
        summary: "partial work done".to_string(),
        tokens_used: 5000,
    };

    match outcome {
        AgentOutcome::TokenBudgetExhausted {
            summary,
            tokens_used,
        } => {
            assert_eq!(summary, "partial work done");
            assert_eq!(tokens_used, 5000);
        }
        _ => panic!("Expected TokenBudgetExhausted variant"),
    }
}

#[tokio::test]
async fn test_mock_agent_run() {
    // P1d.AC2.1 & P1d.AC2.3: Verify mock agent can be run
    let profile = AgentProfile {
        name: "test".to_string(),
        extends: None,
        role: "test role".to_string(),
        system_prompt: "test prompt".to_string(),
        allowed_tools: vec![],
        security: SecurityScope::default(),
        llm: Default::default(),
        turn_limit: None,
        token_budget: None,
    };

    let agent = MockAgent {
        id: "agent-1".to_string(),
        profile,
    };

    let ctx = AgentContext {
        work_package_tasks: vec![],
        relevant_decisions: vec![],
        handoff_notes: None,
        agents_md_summaries: vec![],
        profile: agent.profile().clone(),
        project_path: PathBuf::from("/tmp"),
        graph_store: Arc::new(MockGraphStore),
    };

    let result = agent.run(ctx).await;
    assert!(result.is_ok());

    match result.unwrap() {
        AgentOutcome::Completed { summary } => {
            assert_eq!(summary, "mock completed");
        }
        _ => panic!("Expected Completed outcome"),
    }
}

// Mock GraphStore for testing
struct MockGraphStore;

#[async_trait]
impl rustagent::graph::store::GraphStore for MockGraphStore {
    async fn create_node(&self, _node: &GraphNode) -> anyhow::Result<()> {
        Ok(())
    }

    async fn update_node(
        &self,
        _id: &str,
        _status: Option<NodeStatus>,
        _title: Option<&str>,
        _description: Option<&str>,
        _metadata: Option<&HashMap<String, String>>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_node(&self, _id: &str) -> anyhow::Result<Option<GraphNode>> {
        Ok(None)
    }

    async fn query_nodes(
        &self,
        _query: &rustagent::graph::store::NodeQuery,
    ) -> anyhow::Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn claim_task(&self, _node_id: &str, _agent_id: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn get_ready_tasks(&self, _goal_id: &str) -> anyhow::Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_next_task(&self, _goal_id: &str) -> anyhow::Result<Option<GraphNode>> {
        Ok(None)
    }

    async fn add_edge(&self, _edge: &rustagent::graph::GraphEdge) -> anyhow::Result<()> {
        Ok(())
    }

    async fn remove_edge(&self, _edge_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_edges(
        &self,
        _node_id: &str,
        _direction: rustagent::graph::store::EdgeDirection,
    ) -> anyhow::Result<Vec<(rustagent::graph::GraphEdge, GraphNode)>> {
        Ok(vec![])
    }

    async fn get_children(&self, _node_id: &str) -> anyhow::Result<Vec<(GraphNode, EdgeType)>> {
        Ok(vec![])
    }

    async fn get_subtree(&self, _node_id: &str) -> anyhow::Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_active_decisions(&self, _project_id: &str) -> anyhow::Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn get_full_graph(
        &self,
        _goal_id: &str,
    ) -> anyhow::Result<rustagent::graph::store::WorkGraph> {
        Ok(rustagent::graph::store::WorkGraph {
            nodes: vec![],
            edges: vec![],
        })
    }

    async fn search_nodes(
        &self,
        _query: &str,
        _project_id: Option<&str>,
        _node_type: Option<rustagent::graph::NodeType>,
        _limit: usize,
    ) -> anyhow::Result<Vec<GraphNode>> {
        Ok(vec![])
    }

    async fn next_child_seq(&self, _parent_id: &str) -> anyhow::Result<u32> {
        Ok(1)
    }
}
