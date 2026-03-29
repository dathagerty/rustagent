use rustagent::agent::runtime::{AgentRuntime, RuntimeConfig};
use rustagent::agent::{AgentContext, AgentOutcome, AgentProfile};
use rustagent::llm::mock::MockLlmClient;
use rustagent::security::SecurityScope;
use rustagent::tools::ToolRegistry;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

mod common;
use common::MockGraphStore;

// Helper to create a mock agent context
fn make_test_context() -> AgentContext {
    AgentContext {
        work_package_tasks: vec![],
        relevant_decisions: vec![],
        handoff_notes: None,
        agents_md_summaries: vec![],
        profile: AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test agent".to_string(),
            system_prompt: "You are a test agent.".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: None,
            token_budget: None,
        },
        project_path: PathBuf::from("/tmp/test"),
        graph_store: Arc::new(MockGraphStore),
        previous_attempt: None,
        dependency_statuses: vec![],
    }
}

#[tokio::test]
async fn test_p1d_ac4_1_simple_completion() {
    // P1d.AC4.1: AgentRuntime runs LLM -> tool execution loop and returns Completed on signal_completion
    let mock_client = Arc::new(MockLlmClient::new());

    // Queue responses: first a text response, then signal_completion
    mock_client.queue_text_response("I'll help you with this task.");
    mock_client.queue_tool_call(
        "signal_completion",
        json!({
            "signal": "complete",
            "message": "Task completed successfully"
        }),
    );

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(100),
            token_budget: Some(200_000),
        },
        RuntimeConfig::default(),
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    match outcome {
        AgentOutcome::Completed { summary, .. } => {
            assert!(summary.contains("Task completed successfully"));
        }
        _ => panic!("Expected Completed outcome, got {:?}", outcome),
    }
}

#[tokio::test]
async fn test_p1d_ac4_2_confusion_counter() {
    // P1d.AC4.2: Consecutive bad tool calls (confusion counter) should return Blocked
    let mock_client = Arc::new(MockLlmClient::new());

    // Queue 3 consecutive bad tool calls (unknown tool name)
    for _ in 0..3 {
        mock_client.queue_tool_call("unknown_tool", json!({"param": "value"}));
    }

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let mut config = RuntimeConfig::default();
    config.max_consecutive_tool_failures = 2; // Lower threshold for testing

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(100),
            token_budget: Some(200_000),
        },
        config,
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    match outcome {
        AgentOutcome::Blocked { reason } => {
            assert!(reason.contains("tool") || reason.contains("failure"));
        }
        _ => panic!("Expected Blocked outcome, got {:?}", outcome),
    }
}

#[tokio::test]
async fn test_p1d_ac4_3_token_budget_warning() {
    // P1d.AC4.3: At warning threshold (80%), inject "wrap up" message
    let mock_client = Arc::new(MockLlmClient::new());

    // set_token_counts is global (applies to all responses), not per-response
    // First call returns 800 tokens total (400+400), reaching 80% of 1000 budget
    mock_client.set_token_counts(400, 400);
    mock_client.queue_text_response("Processing...");

    // Second call: wrap-up message should be injected before this call
    mock_client.queue_tool_call(
        "signal_completion",
        json!({
            "signal": "complete",
            "message": "Done"
        }),
    );

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let mut config = RuntimeConfig::default();
    config.token_budget = 1000;
    config.token_budget_warning_pct = 80;

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(100),
            token_budget: Some(1000),
        },
        config,
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    // Verify that the wrap-up logic was engaged by checking recorded LLM calls
    let calls = mock_client.get_recorded_calls();
    assert!(calls.len() >= 2, "Expected at least 2 LLM calls");

    // The second call's messages should contain the wrap-up warning injected by the runtime
    let second_call_messages = &calls[1].0;
    let has_wrap_up_message = second_call_messages
        .iter()
        .any(|msg| msg.role == rustagent::llm::Role::System && msg.content.contains("Wrap up"));
    assert!(
        has_wrap_up_message,
        "Expected wrap-up system message in second LLM call messages"
    );

    // Verify outcome is valid completion or token exhaustion
    match outcome {
        AgentOutcome::Completed { .. } | AgentOutcome::TokenBudgetExhausted { .. } => {}
        _ => panic!("Unexpected outcome: {:?}", outcome),
    }
}

#[tokio::test]
async fn test_p1d_ac4_3_token_budget_exhausted() {
    // P1d.AC4.3: At 100% budget, return TokenBudgetExhausted
    let mock_client = Arc::new(MockLlmClient::new());

    // First call uses all remaining budget
    mock_client.set_token_counts(600, 400); // Total 1000 of budget 1000
    mock_client.queue_text_response("Using up budget");

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let mut config = RuntimeConfig::default();
    config.token_budget = 1000;

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(100),
            token_budget: Some(1000),
        },
        config,
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    match outcome {
        AgentOutcome::TokenBudgetExhausted { tokens_used, .. } => {
            assert_eq!(tokens_used, 1000);
        }
        _ => panic!("Expected TokenBudgetExhausted, got {:?}", outcome),
    }
}

#[tokio::test]
async fn test_p1d_ac4_4_llm_failure_threshold() {
    // P1d.AC4.4: After N consecutive LLM failures, worker signals blocked
    let mock_client = Arc::new(MockLlmClient::new());

    // Queue 3 errors (consecutive LLM failures)
    for _ in 0..3 {
        // We'll simulate LLM errors by queueing nothing and then trying to use it
        // Actually, the mock client will return an error if no response is queued
        // Let's not queue any responses so chat() will error
    }

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let mut config = RuntimeConfig::default();
    config.max_consecutive_llm_failures = 2; // Lower threshold for testing

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(100),
            token_budget: Some(200_000),
        },
        config,
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    match outcome {
        AgentOutcome::Blocked { reason } => {
            assert!(reason.contains("LLM") || reason.contains("llm") || reason.contains("failure"));
        }
        _ => panic!("Expected Blocked outcome, got {:?}", outcome),
    }
}

#[tokio::test]
async fn test_p1d_ac4_5_turn_limit() {
    // P1d.AC4.5: After max_turns, return Completed with "turn limit reached"
    let mock_client = Arc::new(MockLlmClient::new());

    // Queue 4 text responses (more than max_turns)
    for _ in 0..4 {
        mock_client.queue_text_response("Continuing work...");
    }

    let registry = ToolRegistry::new();
    registry.register(Arc::new(rustagent::tools::signal::SignalTool::new()));

    let mut config = RuntimeConfig::default();
    config.max_turns = 3;

    let runtime = AgentRuntime::new(
        mock_client.clone(),
        registry,
        AgentProfile {
            name: "test".to_string(),
            extends: None,
            role: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            allowed_tools: vec!["signal_completion".to_string()],
            security: SecurityScope::default(),
            llm: Default::default(),
            turn_limit: Some(3),
            token_budget: None,
        },
        config,
    );

    let ctx = make_test_context();
    let outcome = runtime.run(ctx).await.expect("Runtime failed");

    match outcome {
        AgentOutcome::Completed { summary, .. } => {
            assert!(summary.contains("turn") || summary.contains("limit"));
        }
        _ => panic!(
            "Expected Completed outcome with turn limit message, got {:?}",
            outcome
        ),
    }
}
