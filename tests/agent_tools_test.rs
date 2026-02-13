use rustagent::graph::store::GraphStore;
use rustagent::graph::{NodeStatus, NodeType};
use rustagent::message::{MessageBus, TokioMessageBus};
use rustagent::tools::Tool;
use rustagent::tools::agent_tools::{QueryAgentStatusTool, SendMessageTool, SpawnSubAgentTool};
use serde_json::json;
use std::sync::Arc;

mod common;

// ===== SpawnSubAgentTool Tests =====

/// P2e.AC1.1: Tool creates a new task node under the parent task
#[tokio::test]
async fn test_spawn_sub_agent_creates_child_node() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    // Create a parent goal and task
    let goal = common::create_test_goal("ra-test", "proj-1", "Test goal");
    graph_store.create_node(&goal).await.unwrap();

    let task =
        common::create_test_task("ra-test.1", "proj-1", "Parent task", NodeStatus::InProgress);
    graph_store.create_node(&task).await.unwrap();

    let tool = SpawnSubAgentTool::new(graph_store.clone(), message_bus, "worker-1".to_string());

    let result = tool
        .execute(json!({
            "title": "Sub-task A",
            "description": "Handle the sub-work",
            "parent_task_id": "ra-test.1",
            "profile": "coder",
            "file_scope": "src/main.rs,src/lib.rs"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    // P2e.AC1.3: Return value contains the new task_id
    let task_id = parsed["task_id"].as_str().unwrap();
    assert!(task_id.starts_with("ra-test.1."));

    // P2e.AC1.1: Node exists in graph store as child of parent
    let child_node = graph_store.get_node(task_id).await.unwrap().unwrap();
    assert_eq!(child_node.node_type, NodeType::Task);
    assert_eq!(child_node.title, "Sub-task A");
    assert_eq!(child_node.description, "Handle the sub-work");
    assert_eq!(child_node.project_id, "proj-1");

    // P2e.AC1.4: New task has status Ready
    assert_eq!(child_node.status, NodeStatus::Ready);

    // Metadata preserved
    assert_eq!(child_node.metadata.get("profile").unwrap(), "coder");
    assert_eq!(
        child_node.metadata.get("file_scope").unwrap(),
        "src/main.rs,src/lib.rs"
    );
    assert_eq!(child_node.metadata.get("spawned_by").unwrap(), "worker-1");
}

/// P2e.AC1.2: Tool sends NodeCreated message to orchestrator
#[tokio::test]
async fn test_spawn_sub_agent_broadcasts_message() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    // Subscribe the orchestrator before spawning
    let mut rx = message_bus.subscribe(&"orchestrator".to_string());

    let goal = common::create_test_goal("ra-test", "proj-1", "Test goal");
    graph_store.create_node(&goal).await.unwrap();
    let task = common::create_test_task("ra-test.1", "proj-1", "Parent", NodeStatus::InProgress);
    graph_store.create_node(&task).await.unwrap();

    let tool = SpawnSubAgentTool::new(
        graph_store.clone(),
        message_bus.clone(),
        "worker-1".to_string(),
    );

    tool.execute(json!({
        "title": "Broadcast test",
        "description": "Test broadcast",
        "parent_task_id": "ra-test.1"
    }))
    .await
    .unwrap();

    // Should receive NodeCreated message
    let msg = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        rustagent::message::WorkerMessage::NodeCreated {
            agent_id,
            parent_id,
            node,
        } => {
            assert_eq!(agent_id, "worker-1");
            assert_eq!(parent_id, "ra-test.1");
            assert_eq!(node.node_type, NodeType::Task);
            assert_eq!(node.status, NodeStatus::Ready);
        }
        other => panic!("Expected NodeCreated, got {:?}", other),
    }
}

/// P2e.AC1: spawn_sub_agent with non-existent parent returns error
#[tokio::test]
async fn test_spawn_sub_agent_missing_parent() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    let tool = SpawnSubAgentTool::new(graph_store, message_bus, "worker-1".to_string());

    let result = tool
        .execute(json!({
            "title": "Orphan task",
            "description": "No parent",
            "parent_task_id": "ra-nonexistent"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed["error"].as_str().unwrap().contains("not found"));
}

/// P2e.AC1: Default profile is "coder" when not specified
#[tokio::test]
async fn test_spawn_sub_agent_default_profile() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    let goal = common::create_test_goal("ra-test", "proj-1", "Test goal");
    graph_store.create_node(&goal).await.unwrap();
    let task = common::create_test_task("ra-test.1", "proj-1", "Parent", NodeStatus::InProgress);
    graph_store.create_node(&task).await.unwrap();

    let tool = SpawnSubAgentTool::new(graph_store.clone(), message_bus, "worker-1".to_string());

    let result = tool
        .execute(json!({
            "title": "Default profile test",
            "description": "No profile specified",
            "parent_task_id": "ra-test.1"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let task_id = parsed["task_id"].as_str().unwrap();
    let node = graph_store.get_node(task_id).await.unwrap().unwrap();
    assert_eq!(node.metadata.get("profile").unwrap(), "coder");
}

// ===== SendMessageTool Tests =====

/// P2e.AC2.1: Send message to a subscribed agent
#[tokio::test]
async fn test_send_message_to_subscribed_agent() {
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    // Subscribe the target agent
    let mut rx = message_bus.subscribe(&"worker-2".to_string());

    let tool = SendMessageTool::new(message_bus.clone());

    let result = tool
        .execute(json!({
            "target_agent_id": "worker-2",
            "message_type": "additional_context",
            "content": "Here is some extra context for you"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"].as_str().unwrap(), "sent");

    // Verify the message was received
    let msg = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        rustagent::message::WorkerMessage::AdditionalContext { content } => {
            assert_eq!(content, "Here is some extra context for you");
        }
        other => panic!("Expected AdditionalContext, got {:?}", other),
    }
}

/// P2e.AC2.2: Send review_request message
#[tokio::test]
async fn test_send_review_request() {
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());
    let mut rx = message_bus.subscribe(&"reviewer-1".to_string());

    let tool = SendMessageTool::new(message_bus.clone());

    let result = tool
        .execute(json!({
            "target_agent_id": "reviewer-1",
            "message_type": "review_request",
            "content": {
                "work_package_id": "wp-123",
                "changed_files": ["src/main.rs", "src/lib.rs"]
            }
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"].as_str().unwrap(), "sent");

    let msg = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        rustagent::message::WorkerMessage::ReviewRequest {
            work_package_id,
            changed_files,
        } => {
            assert_eq!(work_package_id, "wp-123");
            assert_eq!(changed_files.len(), 2);
        }
        other => panic!("Expected ReviewRequest, got {:?}", other),
    }
}

/// P2e.AC2.2: Send review_feedback message
#[tokio::test]
async fn test_send_review_feedback() {
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());
    let mut rx = message_bus.subscribe(&"worker-1".to_string());

    let tool = SendMessageTool::new(message_bus.clone());

    let result = tool
        .execute(json!({
            "target_agent_id": "worker-1",
            "message_type": "review_feedback",
            "content": {
                "approved": true,
                "comments": ["Looks good", "Clean implementation"]
            }
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"].as_str().unwrap(), "sent");

    let msg = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .unwrap()
        .unwrap();

    match msg {
        rustagent::message::WorkerMessage::ReviewFeedback { approved, comments } => {
            assert!(approved);
            assert_eq!(comments.len(), 2);
            assert_eq!(comments[0], "Looks good");
        }
        other => panic!("Expected ReviewFeedback, got {:?}", other),
    }
}

/// P2e.AC2.3: Send to non-existent agent returns error string (not crash)
#[tokio::test]
async fn test_send_message_to_nonexistent_agent() {
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    let tool = SendMessageTool::new(message_bus.clone());

    let result = tool
        .execute(json!({
            "target_agent_id": "ghost-agent",
            "message_type": "additional_context",
            "content": "Hello?"
        }))
        .await
        .unwrap(); // Should not panic

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(parsed["error"].as_str().unwrap().contains("ghost-agent"));
}

/// P2e.AC2: Unknown message type returns error
#[tokio::test]
async fn test_send_message_unknown_type() {
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    let tool = SendMessageTool::new(message_bus.clone());

    let result = tool
        .execute(json!({
            "target_agent_id": "worker-1",
            "message_type": "invalid_type",
            "content": "test"
        }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        parsed["error"]
            .as_str()
            .unwrap()
            .contains("unknown message_type")
    );
}

// ===== QueryAgentStatusTool Tests =====

/// P2e.AC3.1: Query returns tasks assigned to a given agent
#[tokio::test]
async fn test_query_agent_status_with_tasks() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);

    // Create tasks assigned to worker-1
    let goal = common::create_test_goal("ra-test", "proj-1", "Test goal");
    graph_store.create_node(&goal).await.unwrap();

    let mut task1 =
        common::create_test_task("ra-test.1", "proj-1", "Task A", NodeStatus::InProgress);
    task1.assigned_to = Some("worker-1".to_string());
    graph_store.create_node(&task1).await.unwrap();

    let mut task2 =
        common::create_test_task("ra-test.2", "proj-1", "Task B", NodeStatus::Completed);
    task2.assigned_to = Some("worker-1".to_string());
    graph_store.create_node(&task2).await.unwrap();

    // Task assigned to different worker (should not appear)
    let mut task3 = common::create_test_task("ra-test.3", "proj-1", "Task C", NodeStatus::Ready);
    task3.assigned_to = Some("worker-2".to_string());
    graph_store.create_node(&task3).await.unwrap();

    let tool = QueryAgentStatusTool::new(graph_store);

    let result = tool
        .execute(json!({ "agent_id": "worker-1" }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["agent_id"].as_str().unwrap(), "worker-1");

    let tasks = parsed["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);

    // Verify task details
    let task_ids: Vec<&str> = tasks.iter().filter_map(|t| t["task_id"].as_str()).collect();
    assert!(task_ids.contains(&"ra-test.1"));
    assert!(task_ids.contains(&"ra-test.2"));
}

/// P2e.AC3.2: Query for non-existent agent returns empty list
#[tokio::test]
async fn test_query_agent_status_no_tasks() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);

    let tool = QueryAgentStatusTool::new(graph_store);

    let result = tool
        .execute(json!({ "agent_id": "ghost-agent" }))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["agent_id"].as_str().unwrap(), "ghost-agent");
    assert!(parsed["tasks"].as_array().unwrap().is_empty());
    assert!(parsed["note"].as_str().unwrap().contains("no tasks found"));
}

// ===== Tool Trait Compliance Tests =====

/// P2e.AC4.2: All three tools implement the Tool trait correctly
#[tokio::test]
async fn test_tool_trait_compliance() {
    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    // SpawnSubAgentTool
    let spawn_tool =
        SpawnSubAgentTool::new(graph_store.clone(), message_bus.clone(), "w1".to_string());
    assert_eq!(spawn_tool.name(), "spawn_sub_agent");
    assert!(!spawn_tool.description().is_empty());
    let params = spawn_tool.parameters();
    assert!(params["properties"]["title"].is_object());
    assert!(params["properties"]["parent_task_id"].is_object());

    // SendMessageTool
    let send_tool = SendMessageTool::new(message_bus.clone());
    assert_eq!(send_tool.name(), "send_message");
    assert!(!send_tool.description().is_empty());
    let params = send_tool.parameters();
    assert!(params["properties"]["target_agent_id"].is_object());
    assert!(params["properties"]["message_type"].is_object());

    // QueryAgentStatusTool
    let query_tool = QueryAgentStatusTool::new(graph_store);
    assert_eq!(query_tool.name(), "query_agent_status");
    assert!(!query_tool.description().is_empty());
    let params = query_tool.parameters();
    assert!(params["properties"]["agent_id"].is_object());
}

// ===== Registry Tests =====

/// P2e.AC4.1: Agent tools registered in multi-agent mode
#[tokio::test]
async fn test_registry_includes_agent_tools_in_multi_agent_mode() {
    use rustagent::config::{SecurityConfig, ShellPolicy};
    use rustagent::security::SecurityValidator;
    use rustagent::security::permission::AutoApproveHandler;
    use rustagent::tools::factory::create_v2_registry;

    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);
    let message_bus: Arc<dyn MessageBus> = Arc::new(TokioMessageBus::default());

    let security_config = SecurityConfig {
        shell_policy: ShellPolicy::Unrestricted,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 100,
        allowed_paths: vec![],
    };
    let validator = Arc::new(SecurityValidator::new(security_config).unwrap());
    let permission_handler = Arc::new(AutoApproveHandler);

    let registry = create_v2_registry(
        validator,
        permission_handler,
        graph_store,
        Some(message_bus),
        Some("worker-1".to_string()),
    );

    let tools = registry.list();
    assert!(tools.contains(&"spawn_sub_agent".to_string()));
    assert!(tools.contains(&"send_message".to_string()));
    assert!(tools.contains(&"query_agent_status".to_string()));
}

/// P2e.AC4.2: Agent tools NOT registered in single-agent mode (None, None)
#[tokio::test]
async fn test_registry_excludes_agent_tools_in_single_agent_mode() {
    use rustagent::config::{SecurityConfig, ShellPolicy};
    use rustagent::security::SecurityValidator;
    use rustagent::security::permission::AutoApproveHandler;
    use rustagent::tools::factory::create_v2_registry;

    let (_, graph_store) = common::setup_test_env().await.unwrap();
    let graph_store: Arc<dyn GraphStore> = Arc::new(graph_store);

    let security_config = SecurityConfig {
        shell_policy: ShellPolicy::Unrestricted,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 100,
        allowed_paths: vec![],
    };
    let validator = Arc::new(SecurityValidator::new(security_config).unwrap());
    let permission_handler = Arc::new(AutoApproveHandler);

    let registry = create_v2_registry(validator, permission_handler, graph_store, None, None);

    let tools = registry.list();
    assert!(!tools.contains(&"spawn_sub_agent".to_string()));
    assert!(!tools.contains(&"send_message".to_string()));
    assert!(!tools.contains(&"query_agent_status".to_string()));

    // But still has graph tools
    assert!(tools.contains(&"create_node".to_string()));
    assert!(tools.contains(&"signal_completion".to_string()));
}
