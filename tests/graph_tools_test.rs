use rustagent::config::SecurityConfig;
use rustagent::db::Database;
use rustagent::graph::store::{GraphStore, SqliteGraphStore};
use rustagent::graph::{EdgeType, NodeStatus, NodeType};
use rustagent::security::SecurityValidator;
use rustagent::security::permission::AutoApproveHandler;
use rustagent::tools::Tool;
use rustagent::tools::factory::create_v2_registry;
use rustagent::tools::graph_tools::*;
use serde_json::{Value, json};
use std::sync::Arc;

mod common;
use common::MockGraphStore;

/// Create a test database in memory with a test project
async fn setup_test_db() -> anyhow::Result<(Database, Arc<SqliteGraphStore>)> {
    let db = Database::open_in_memory().await?;

    // Insert a test project
    db.connection()
        .call(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO projects (id, name, path, registered_at, config_overrides, metadata)
                 VALUES (?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    "proj-1",
                    "proj-1",
                    "/tmp/proj-1",
                    &now,
                    None::<String>,
                    "{}"
                ],
            )?;
            Ok(())
        })
        .await?;

    let store = Arc::new(SqliteGraphStore::new(db.clone()));
    Ok((db, store))
}

#[tokio::test]
async fn test_create_node_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let tool = CreateNodeTool::new(store.clone());

    let params = json!({
        "node_type": "task",
        "title": "Test Task",
        "description": "A test task",
        "project_id": "proj-1"
    });

    let result = tool.execute(params).await?;
    let parsed: Value = serde_json::from_str(&result)?;

    assert!(parsed["id"].as_str().is_some());
    assert!(parsed["id"].as_str().unwrap().starts_with("ra-"));
    assert_eq!(parsed["message"], "Node created successfully");

    // Verify node was created
    let node_id = parsed["id"].as_str().unwrap();
    let node = store.get_node(node_id).await?.expect("Node not found");

    assert_eq!(node.title, "Test Task");
    assert_eq!(node.node_type, NodeType::Task);
    assert_eq!(node.status, NodeStatus::Pending);

    Ok(())
}

#[tokio::test]
async fn test_create_child_node_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let tool = CreateNodeTool::new(store.clone());

    // Create parent
    let parent_params = json!({
        "node_type": "goal",
        "title": "Parent Goal",
        "description": "A parent goal",
        "project_id": "proj-1"
    });

    let parent_result = tool.execute(parent_params).await?;
    let parent_parsed: Value = serde_json::from_str(&parent_result)?;
    let parent_id = parent_parsed["id"].as_str().unwrap();

    // Create child with parent_id
    let child_params = json!({
        "node_type": "task",
        "title": "Child Task",
        "description": "A child task",
        "project_id": "proj-1",
        "parent_id": parent_id
    });

    let child_result = tool.execute(child_params).await?;
    let child_parsed: Value = serde_json::from_str(&child_result)?;
    let child_id = child_parsed["id"].as_str().unwrap();

    // Verify child ID has parent prefix
    assert!(child_id.starts_with(parent_id));
    assert!(child_id.contains("."));

    // Verify Contains edge was created
    let edges = store
        .get_edges(parent_id, rustagent::graph::store::EdgeDirection::Outgoing)
        .await?;

    assert!(!edges.is_empty());
    assert_eq!(edges[0].0.edge_type, EdgeType::Contains);

    Ok(())
}

#[tokio::test]
async fn test_update_node_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let update_tool = UpdateNodeTool::new(store.clone());

    // Create a node
    let create_params = json!({
        "node_type": "task",
        "title": "Original Title",
        "description": "Original description",
        "project_id": "proj-1"
    });

    let create_result = create_tool.execute(create_params).await?;
    let parsed: Value = serde_json::from_str(&create_result)?;
    let node_id = parsed["id"].as_str().unwrap();

    // Update the node
    let update_params = json!({
        "node_id": node_id,
        "title": "Updated Title",
        "description": "Updated description",
        "status": "ready"
    });

    update_tool.execute(update_params).await?;

    // Verify update
    let node = store.get_node(node_id).await?.expect("Node not found");

    assert_eq!(node.title, "Updated Title");
    assert_eq!(node.description, "Updated description");
    assert_eq!(node.status, NodeStatus::Ready);

    Ok(())
}

#[tokio::test]
async fn test_add_edge_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let edge_tool = AddEdgeTool::new(store.clone());

    // Create two nodes
    let params1 = json!({
        "node_type": "task",
        "title": "Task 1",
        "description": "First task",
        "project_id": "proj-1"
    });

    let result1 = create_tool.execute(params1).await?;
    let parsed1: Value = serde_json::from_str(&result1)?;
    let node1_id = parsed1["id"].as_str().unwrap();

    let params2 = json!({
        "node_type": "task",
        "title": "Task 2",
        "description": "Second task",
        "project_id": "proj-1"
    });

    let result2 = create_tool.execute(params2).await?;
    let parsed2: Value = serde_json::from_str(&result2)?;
    let node2_id = parsed2["id"].as_str().unwrap();

    // Add DependsOn edge
    let edge_params = json!({
        "edge_type": "depends_on",
        "from_node": node2_id,
        "to_node": node1_id,
        "label": "blocks"
    });

    let edge_result = edge_tool.execute(edge_params).await?;
    let edge_parsed: Value = serde_json::from_str(&edge_result)?;

    assert!(edge_parsed["id"].as_str().is_some());
    assert_eq!(edge_parsed["message"], "Edge created successfully");

    // Verify edge
    let edges = store
        .get_edges(node2_id, rustagent::graph::store::EdgeDirection::Outgoing)
        .await?;

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0.edge_type, EdgeType::DependsOn);

    Ok(())
}

#[tokio::test]
async fn test_query_nodes_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let query_tool = QueryNodesTool::new(store.clone());

    // Create a few nodes
    for i in 1..=3 {
        let params = json!({
            "node_type": "task",
            "title": format!("Task {}", i),
            "description": "Test task",
            "project_id": "proj-1"
        });
        create_tool.execute(params).await?;
    }

    // Query all tasks
    let query_params = json!({
        "node_type": "task",
        "project_id": "proj-1"
    });

    let result = query_tool.execute(query_params).await?;
    let parsed: Vec<Value> = serde_json::from_str(&result)?;

    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0]["node_type"], "task");

    Ok(())
}

#[tokio::test]
async fn test_search_nodes_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let search_tool = SearchNodesTool::new(store.clone());

    // Create nodes with specific titles
    let params = json!({
        "node_type": "task",
        "title": "Authentication Task",
        "description": "Handle user authentication",
        "project_id": "proj-1"
    });
    create_tool.execute(params).await?;

    // Search for "authentication"
    let search_params = json!({
        "query": "authentication"
    });

    let result = search_tool.execute(search_params).await?;
    let parsed: Vec<Value> = serde_json::from_str(&result)?;

    assert!(!parsed.is_empty());
    assert_eq!(parsed[0]["title"], "Authentication Task");

    Ok(())
}

#[tokio::test]
async fn test_claim_task_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let update_tool = UpdateNodeTool::new(store.clone());
    let claim_tool = ClaimTaskTool::new(store.clone());

    // Create a task
    let create_params = json!({
        "node_type": "task",
        "title": "Test Task",
        "description": "To be claimed",
        "project_id": "proj-1"
    });

    let create_result = create_tool.execute(create_params).await?;
    let parsed: Value = serde_json::from_str(&create_result)?;
    let task_id = parsed["id"].as_str().unwrap();

    // Update status to Ready
    let update_params = json!({
        "node_id": task_id,
        "status": "ready"
    });
    update_tool.execute(update_params).await?;

    // Claim the task
    let claim_params = json!({
        "node_id": task_id,
        "agent_id": "agent-1"
    });

    let claim_result = claim_tool.execute(claim_params).await?;
    let claim_parsed: Value = serde_json::from_str(&claim_result)?;

    assert_eq!(claim_parsed["claimed"], true);

    // Verify node status
    let node = store.get_node(task_id).await?.expect("Node not found");

    assert_eq!(node.status, NodeStatus::Claimed);
    assert_eq!(node.assigned_to, Some("agent-1".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_log_decision_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let tool = LogDecisionTool::new(store.clone());

    let params = json!({
        "title": "Architecture Decision",
        "description": "Choose between microservices or monolith",
        "project_id": "proj-1",
        "options": [
            {
                "title": "Microservices",
                "description": "Multiple independent services",
                "pros": "Scalability, independence",
                "cons": "Complexity, latency"
            },
            {
                "title": "Monolith",
                "description": "Single unified application",
                "pros": "Simplicity, performance",
                "cons": "Scalability limitations"
            }
        ]
    });

    let result = tool.execute(params).await?;
    let parsed: Value = serde_json::from_str(&result)?;

    assert!(parsed["decision_id"].as_str().is_some());
    assert!(parsed["option_ids"].is_array());
    assert_eq!(parsed["option_ids"].as_array().unwrap().len(), 2);

    // Verify decision node was created
    let decision_id = parsed["decision_id"].as_str().unwrap();
    let decision = store
        .get_node(decision_id)
        .await?
        .expect("Decision not found");

    assert_eq!(decision.node_type, NodeType::Decision);
    assert_eq!(decision.status, NodeStatus::Active);

    // Verify option nodes were created
    let option_ids = parsed["option_ids"].as_array().unwrap();
    for option_id_val in option_ids {
        let option_id = option_id_val.as_str().unwrap();
        let option = store.get_node(option_id).await?.expect("Option not found");

        assert_eq!(option.node_type, NodeType::Option);
        assert_eq!(option.status, NodeStatus::Active);
    }

    Ok(())
}

#[tokio::test]
async fn test_choose_option_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let decision_tool = LogDecisionTool::new(store.clone());
    let choose_tool = ChooseOptionTool::new(store.clone());

    // Create a decision with options
    let decision_params = json!({
        "title": "Test Decision",
        "description": "Test",
        "project_id": "proj-1",
        "options": [
            {
                "title": "Option A",
                "description": "First option"
            },
            {
                "title": "Option B",
                "description": "Second option"
            }
        ]
    });

    let decision_result = decision_tool.execute(decision_params).await?;
    let decision_parsed: Value = serde_json::from_str(&decision_result)?;

    let decision_id = decision_parsed["decision_id"].as_str().unwrap();
    let option_ids = decision_parsed["option_ids"].as_array().unwrap();
    let chosen_option_id = option_ids[0].as_str().unwrap();

    // Choose an option
    let choose_params = json!({
        "decision_id": decision_id,
        "option_id": chosen_option_id,
        "rationale": "Best fit for our needs"
    });

    choose_tool.execute(choose_params).await?;

    // Verify decision status changed to Decided
    let decision = store
        .get_node(decision_id)
        .await?
        .expect("Decision not found");

    assert_eq!(decision.status, NodeStatus::Decided);

    // Verify chosen option has Chosen status
    let chosen_option = store
        .get_node(chosen_option_id)
        .await?
        .expect("Option not found");

    assert_eq!(chosen_option.status, NodeStatus::Chosen);

    // Verify other options are Rejected
    let other_option_id = option_ids[1].as_str().unwrap();
    let other_option = store
        .get_node(other_option_id)
        .await?
        .expect("Option not found");

    assert_eq!(other_option.status, NodeStatus::Rejected);

    Ok(())
}

#[tokio::test]
async fn test_record_outcome_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let outcome_tool = RecordOutcomeTool::new(store.clone());

    // Create a task
    let task_params = json!({
        "node_type": "task",
        "title": "Test Task",
        "description": "Task to record outcome for",
        "project_id": "proj-1"
    });

    let task_result = create_tool.execute(task_params).await?;
    let task_parsed: Value = serde_json::from_str(&task_result)?;
    let task_id = task_parsed["id"].as_str().unwrap();

    // Record outcome
    let outcome_params = json!({
        "parent_id": task_id,
        "title": "Task Completed",
        "description": "Successfully completed the task",
        "project_id": "proj-1",
        "success": true
    });

    let outcome_result = outcome_tool.execute(outcome_params).await?;
    let outcome_parsed: Value = serde_json::from_str(&outcome_result)?;

    assert!(outcome_parsed["outcome_id"].as_str().is_some());

    // Verify outcome node
    let outcome_id = outcome_parsed["outcome_id"].as_str().unwrap();
    let outcome = store
        .get_node(outcome_id)
        .await?
        .expect("Outcome not found");

    assert_eq!(outcome.node_type, NodeType::Outcome);
    assert_eq!(outcome.status, NodeStatus::Completed);
    assert_eq!(outcome.metadata.get("success"), Some(&"true".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_record_observation_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let obs_tool = RecordObservationTool::new(store.clone());

    // Create a task to observe
    let task_params = json!({
        "node_type": "task",
        "title": "Test Task",
        "description": "Task to observe",
        "project_id": "proj-1"
    });

    let task_result = create_tool.execute(task_params).await?;
    let task_parsed: Value = serde_json::from_str(&task_result)?;
    let task_id = task_parsed["id"].as_str().unwrap();

    // Record observation related to task
    let obs_params = json!({
        "title": "Performance Issue Observed",
        "description": "Task took longer than expected",
        "project_id": "proj-1",
        "related_node_id": task_id
    });

    let obs_result = obs_tool.execute(obs_params).await?;
    let obs_parsed: Value = serde_json::from_str(&obs_result)?;

    assert!(obs_parsed["observation_id"].as_str().is_some());

    // Verify observation node
    let obs_id = obs_parsed["observation_id"].as_str().unwrap();
    let obs = store
        .get_node(obs_id)
        .await?
        .expect("Observation not found");

    assert_eq!(obs.node_type, NodeType::Observation);
    assert_eq!(obs.status, NodeStatus::Active);

    // Verify Informs edge
    let edges = store
        .get_edges(obs_id, rustagent::graph::store::EdgeDirection::Outgoing)
        .await?;

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0.edge_type, EdgeType::Informs);

    Ok(())
}

#[tokio::test]
async fn test_revisit_tool() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;
    let create_tool = CreateNodeTool::new(store.clone());
    let outcome_tool = RecordOutcomeTool::new(store.clone());
    let revisit_tool = RevisitTool::new(store.clone());

    // Create a task and outcome
    let task_params = json!({
        "node_type": "task",
        "title": "Test Task",
        "description": "Task",
        "project_id": "proj-1"
    });

    let task_result = create_tool.execute(task_params).await?;
    let task_parsed: Value = serde_json::from_str(&task_result)?;
    let task_id = task_parsed["id"].as_str().unwrap();

    let outcome_params = json!({
        "parent_id": task_id,
        "title": "Outcome",
        "description": "Task outcome",
        "project_id": "proj-1",
        "success": true
    });

    let outcome_result = outcome_tool.execute(outcome_params).await?;
    let outcome_parsed: Value = serde_json::from_str(&outcome_result)?;
    let outcome_id = outcome_parsed["outcome_id"].as_str().unwrap();

    // Revisit with new decision
    let revisit_params = json!({
        "outcome_id": outcome_id,
        "project_id": "proj-1",
        "reason": "Results not as expected",
        "new_decision_title": "Reconsider approach"
    });

    let revisit_result = revisit_tool.execute(revisit_params).await?;
    let revisit_parsed: Value = serde_json::from_str(&revisit_result)?;

    assert!(revisit_parsed["revisit_id"].as_str().is_some());
    assert!(revisit_parsed["decision_id"].as_str().is_some());

    // Verify revisit node
    let revisit_id = revisit_parsed["revisit_id"].as_str().unwrap();
    let revisit = store
        .get_node(revisit_id)
        .await?
        .expect("Revisit not found");

    assert_eq!(revisit.node_type, NodeType::Revisit);
    assert_eq!(revisit.status, NodeStatus::Active);

    // Verify new decision was created
    let decision_id = revisit_parsed["decision_id"].as_str().unwrap();
    let decision = store
        .get_node(decision_id)
        .await?
        .expect("Decision not found");

    assert_eq!(decision.node_type, NodeType::Decision);

    Ok(())
}

#[tokio::test]
async fn test_tool_name_and_description() -> anyhow::Result<()> {
    let (_db, store) = setup_test_db().await?;

    let tools: Vec<(Box<dyn Tool + Send + Sync>, &str)> = vec![
        (Box::new(CreateNodeTool::new(store.clone())), "create_node"),
        (Box::new(UpdateNodeTool::new(store.clone())), "update_node"),
        (Box::new(AddEdgeTool::new(store.clone())), "add_edge"),
        (Box::new(QueryNodesTool::new(store.clone())), "query_nodes"),
        (
            Box::new(SearchNodesTool::new(store.clone())),
            "search_nodes",
        ),
        (Box::new(ClaimTaskTool::new(store.clone())), "claim_task"),
        (
            Box::new(LogDecisionTool::new(store.clone())),
            "log_decision",
        ),
        (
            Box::new(ChooseOptionTool::new(store.clone())),
            "choose_option",
        ),
        (
            Box::new(RecordOutcomeTool::new(store.clone())),
            "record_outcome",
        ),
        (
            Box::new(RecordObservationTool::new(store.clone())),
            "record_observation",
        ),
        (Box::new(RevisitTool::new(store.clone())), "revisit"),
    ];

    for (tool, expected_name) in tools {
        assert_eq!(tool.name(), expected_name);
        assert!(!tool.description().is_empty());
        let params = tool.parameters();
        assert!(params.is_object());
    }

    Ok(())
}

#[test]
fn test_v2_registry_includes_all_tools() {
    // Create a mock graph store
    let graph_store = Arc::new(MockGraphStore);

    // Create security config and validator
    let security_config = SecurityConfig {
        shell_policy: rustagent::config::ShellPolicy::Blocklist,
        allowed_commands: vec![],
        blocked_patterns: vec![],
        max_file_size_mb: 100,
        allowed_paths: vec![],
    };
    let validator =
        Arc::new(SecurityValidator::new(security_config).expect("Failed to create validator"));
    let permission_handler = Arc::new(AutoApproveHandler);

    // Create the v2 registry
    let registry = create_v2_registry(validator, permission_handler, graph_store, None, None);

    // Expected tool names: graph tools + legacy tools + context tools
    let expected_tools = vec![
        // Graph tools
        "create_node",
        "update_node",
        "add_edge",
        "query_nodes",
        "search_nodes",
        "claim_task",
        "log_decision",
        "choose_option",
        "record_outcome",
        "record_observation",
        "revisit",
        // Legacy tools
        "read_file",
        "write_file",
        "list_files",
        "run_command",
        "signal_completion",
        // Context tools
        "read_agents_md",
    ];

    // Get all registered tool names
    let registered_names = registry.list();

    // Verify each expected tool is registered
    for expected in expected_tools {
        assert!(
            registered_names.contains(&expected.to_string()),
            "Tool '{}' not found in v2 registry. Registered tools: {:?}",
            expected,
            registered_names
        );
    }
}
