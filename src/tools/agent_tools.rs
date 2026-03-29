use crate::agent::AgentId;
use crate::graph::store::GraphStore;
use crate::graph::{GraphNode, NodeStatus, NodeType, generate_child_id};
use crate::message::{MessageBus, WorkerMessage};
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

// ===== SpawnSubAgentTool =====

/// Tool for workers to request sub-agent spawning.
///
/// Creates a new Task node under the caller's task and notifies the
/// orchestrator via the message bus. The orchestrator picks it up in
/// its next scheduling pass.
pub struct SpawnSubAgentTool {
    graph_store: Arc<dyn GraphStore>,
    message_bus: Arc<dyn MessageBus>,
    agent_id: AgentId,
}

impl SpawnSubAgentTool {
    pub fn new(
        graph_store: Arc<dyn GraphStore>,
        message_bus: Arc<dyn MessageBus>,
        agent_id: AgentId,
    ) -> Self {
        Self {
            graph_store,
            message_bus,
            agent_id,
        }
    }
}

#[async_trait]
impl Tool for SpawnSubAgentTool {
    fn name(&self) -> &str {
        "spawn_sub_agent"
    }

    fn description(&self) -> &str {
        "Request a sub-agent to handle a subtask. Creates a new task node and notifies the orchestrator."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title for the new subtask"
                },
                "description": {
                    "type": "string",
                    "description": "What the sub-agent should do"
                },
                "parent_task_id": {
                    "type": "string",
                    "description": "The calling worker's task ID (new task becomes a child)"
                },
                "profile": {
                    "type": "string",
                    "description": "Agent profile for the sub-agent (default: coder)",
                    "enum": ["planner", "coder", "reviewer", "tester", "researcher"]
                },
                "file_scope": {
                    "type": "string",
                    "description": "Comma-separated list of files the sub-agent will need"
                }
            },
            "required": ["title", "description", "parent_task_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let title = params["title"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: title"))?;
        let description = params["description"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: description"))?;
        let parent_task_id = params["parent_task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: parent_task_id"))?;
        let profile = params["profile"].as_str().unwrap_or("coder");
        let file_scope = params["file_scope"].as_str().unwrap_or("");

        // Verify parent exists
        let parent = self.graph_store.get_node(parent_task_id).await?;
        if parent.is_none() {
            return Ok(json!({
                "error": format!("parent task {} not found", parent_task_id)
            })
            .to_string());
        }
        let parent = parent.unwrap();

        // Generate child ID
        let seq = self.graph_store.next_child_seq(parent_task_id).await?;
        let child_id = generate_child_id(parent_task_id, seq);

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("profile".to_string(), profile.to_string());
        if !file_scope.is_empty() {
            metadata.insert("file_scope".to_string(), file_scope.to_string());
        }
        metadata.insert("spawned_by".to_string(), self.agent_id.clone());

        // Create the task node
        let node = GraphNode {
            id: child_id.clone(),
            project_id: parent.project_id.clone(),
            node_type: NodeType::Task,
            title: title.to_string(),
            description: description.to_string(),
            status: NodeStatus::Ready,
            priority: parent.priority,
            assigned_to: None,
            created_by: Some(self.agent_id.clone()),
            labels: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            blocked_reason: None,
            metadata,
        };

        self.graph_store.create_node(&node).await?;

        // Notify the orchestrator
        let _ = self
            .message_bus
            .broadcast(WorkerMessage::NodeCreated {
                agent_id: self.agent_id.clone(),
                parent_id: parent_task_id.to_string(),
                node: node.clone(),
            })
            .await;

        Ok(json!({
            "task_id": child_id,
            "status": "ready"
        })
        .to_string())
    }
}

// ===== SendMessageTool =====

/// Tool for workers to send messages to other workers or the orchestrator.
///
/// Supports review_request, review_feedback, and additional_context message types.
pub struct SendMessageTool {
    message_bus: Arc<dyn MessageBus>,
}

impl SendMessageTool {
    pub fn new(message_bus: Arc<dyn MessageBus>) -> Self {
        Self { message_bus }
    }
}

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "send_message"
    }

    fn description(&self) -> &str {
        "Send a message to another agent (review requests, feedback, or context sharing)"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target_agent_id": {
                    "type": "string",
                    "description": "The agent to send the message to"
                },
                "message_type": {
                    "type": "string",
                    "enum": ["review_request", "review_feedback", "additional_context"],
                    "description": "Type of message to send"
                },
                "content": {
                    "type": "object",
                    "description": "Message content (varies by message_type)"
                }
            },
            "required": ["target_agent_id", "message_type", "content"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let target = params["target_agent_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: target_agent_id"))?;
        let msg_type = params["message_type"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: message_type"))?;
        let content = &params["content"];

        let message = match msg_type {
            "review_request" => {
                let work_package_id = content["work_package_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let changed_files: Vec<std::path::PathBuf> = content["changed_files"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(std::path::PathBuf::from))
                            .collect()
                    })
                    .unwrap_or_default();

                WorkerMessage::ReviewRequest {
                    work_package_id,
                    changed_files,
                }
            }
            "review_feedback" => {
                let approved = content["approved"].as_bool().unwrap_or(false);
                let comments: Vec<String> = content["comments"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                WorkerMessage::ReviewFeedback { approved, comments }
            }
            "additional_context" => {
                let ctx = content
                    .as_str()
                    .or_else(|| content["content"].as_str())
                    .unwrap_or("")
                    .to_string();

                WorkerMessage::AdditionalContext { content: ctx }
            }
            other => {
                return Ok(json!({
                    "error": format!("unknown message_type: {}. Use review_request, review_feedback, or additional_context", other)
                }).to_string());
            }
        };

        match self.message_bus.send(&target.to_string(), message).await {
            Ok(()) => Ok(json!({
                "status": "sent",
                "target": target
            })
            .to_string()),
            Err(e) => Ok(json!({
                "error": format!("failed to send message to {}: {}", target, e)
            })
            .to_string()),
        }
    }
}

// ===== QueryAgentStatusTool =====

/// Tool for workers to query the status of tasks assigned to another agent.
pub struct QueryAgentStatusTool {
    graph_store: Arc<dyn GraphStore>,
}

impl QueryAgentStatusTool {
    pub fn new(graph_store: Arc<dyn GraphStore>) -> Self {
        Self { graph_store }
    }
}

#[async_trait]
impl Tool for QueryAgentStatusTool {
    fn name(&self) -> &str {
        "query_agent_status"
    }

    fn description(&self) -> &str {
        "Query the current status of tasks assigned to an agent"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent_id": {
                    "type": "string",
                    "description": "The agent whose task status to query"
                }
            },
            "required": ["agent_id"]
        })
    }

    async fn execute(&self, params: Value) -> Result<String> {
        let agent_id = params["agent_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: agent_id"))?;

        // Query all task nodes assigned to this agent
        let all_tasks = self
            .graph_store
            .query_nodes(&crate::graph::store::NodeQuery {
                node_type: Some(NodeType::Task),
                status: None,
                project_id: None,
                parent_id: None,
                query: None,
            })
            .await?;

        let agent_tasks: Vec<Value> = all_tasks
            .iter()
            .filter(|n| n.assigned_to.as_deref() == Some(agent_id))
            .map(|n| {
                json!({
                    "task_id": n.id,
                    "status": n.status.to_string(),
                    "title": n.title
                })
            })
            .collect();

        if agent_tasks.is_empty() {
            Ok(json!({
                "agent_id": agent_id,
                "tasks": [],
                "note": "no tasks found for this agent"
            })
            .to_string())
        } else {
            Ok(json!({
                "agent_id": agent_id,
                "tasks": agent_tasks
            })
            .to_string())
        }
    }
}
