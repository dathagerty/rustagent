use async_trait::async_trait;
use rustagent::agent::profile::AgentProfile;
use rustagent::agent::{Agent, AgentContext, AgentId, AgentOutcome};
use rustagent::security::SecurityScope;
use std::path::PathBuf;
use std::sync::Arc;

mod common;
use common::MockGraphStore;

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
