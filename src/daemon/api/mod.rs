pub mod agents;
pub mod graph;
pub mod projects;
pub mod search;

use crate::db::Database;
use crate::graph::store::GraphStore;
use crate::message::MessageBus;
use crate::project::ProjectStore;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// WebSocket event types matching the architecture specification.
///
/// Emission sources:
/// - AgentSpawned, AgentProgress, AgentCompleted: Bridged from WorkerMessage via MessageBus
/// - NodeCreated: Bridged from WorkerMessage::NodeCreated via MessageBus
/// - NodeStatusChanged: Deferred — requires hooks in GraphStore::update_node
/// - EdgeCreated: Deferred — requires hooks in graph mutation
/// - SessionEnded: Deferred — requires orchestrator to emit directly via ws_tx
/// - ToolExecution: Deferred — requires AgentRuntime to emit tool calls
/// - OrchestratorStateChanged: Deferred — requires orchestrator state machine
///
/// In Phase 3, only events bridged from the MessageBus are emitted (the first 4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    AgentSpawned {
        agent_id: String,
        profile: String,
        goal_id: String,
    },
    AgentProgress {
        agent_id: String,
        turn: usize,
        summary: String,
    },
    AgentCompleted {
        agent_id: String,
        outcome_type: String,
        summary: String,
        tokens_used: Option<usize>,
    },
    NodeCreated {
        #[serde(flatten)]
        node: crate::graph::GraphNode,
        parent_id: Option<String>,
    },
    NodeStatusChanged {
        node_id: String,
        node_type: String,
        old_status: String,
        new_status: String,
    },
    EdgeCreated {
        #[serde(flatten)]
        edge: crate::graph::GraphEdge,
    },
    SessionEnded {
        session_id: String,
        handoff_notes: Option<String>,
    },
    ToolExecution {
        agent_id: String,
        tool: String,
        args: serde_json::Value,
        result: String,
    },
    OrchestratorStateChanged {
        goal_id: String,
        state: String,
    },
}

/// Lightweight handle for managing running orchestrators
pub struct OrchestratorHandle {
    pub goal_id: String,
    pub project_id: String,
    pub cancel_token: tokio_util::sync::CancellationToken,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub graph_store: Arc<dyn GraphStore>,
    pub project_store: ProjectStore,
    pub message_bus: Arc<dyn MessageBus>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub orchestrators: Arc<Mutex<HashMap<String, OrchestratorHandle>>>,
}

impl AppState {
    pub fn new(
        db: Database,
        graph_store: Arc<dyn GraphStore>,
        message_bus: Arc<dyn MessageBus>,
    ) -> Self {
        let (ws_tx, _) = broadcast::channel(256);
        Self {
            project_store: ProjectStore::new(db.clone()),
            db,
            graph_store,
            message_bus,
            ws_tx,
            orchestrators: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad request", msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error", msg)
            }
        };

        let body = serde_json::json!({
            "error": error_type,
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}
