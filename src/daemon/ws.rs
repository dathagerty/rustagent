use crate::agent::AgentId;
use crate::daemon::api::{AppState, WsEvent};
use crate::message::{MessageBus, WorkerMessage};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use std::sync::Arc;
use tokio::sync::broadcast;

/// WS /ws — WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.ws_tx.subscribe()))
}

/// Handle an individual WebSocket connection
async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<WsEvent>) {
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let json = match serde_json::to_string(&event) {
                            Ok(j) => j,
                            Err(e) => {
                                tracing::warn!("Failed to serialize WsEvent: {}", e);
                                continue;
                            }
                        };
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WebSocket client lagged, missed {} events", n);
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::debug!("WebSocket client disconnected");
}

/// Bridges WorkerMessage events from the MessageBus to WsEvent broadcasts.
pub fn start_ws_bridge(
    message_bus: Arc<dyn MessageBus>,
    ws_tx: broadcast::Sender<WsEvent>,
) -> tokio::task::JoinHandle<()> {
    let bridge_id: AgentId = "ws-bridge".to_string();
    let mut rx = message_bus.subscribe(&bridge_id);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let event = match msg {
                WorkerMessage::ProgressReport {
                    agent_id,
                    turn,
                    summary,
                } => Some(WsEvent::AgentProgress {
                    agent_id: agent_id.clone(),
                    turn,
                    summary,
                }),
                WorkerMessage::TaskCompleted {
                    agent_id,
                    task_id: _,
                    summary,
                } => Some(WsEvent::AgentCompleted {
                    agent_id: agent_id.clone(),
                    outcome_type: "completed".to_string(),
                    summary,
                    tokens_used: None,
                }),
                WorkerMessage::TaskBlocked {
                    agent_id,
                    task_id: _,
                    reason,
                } => Some(WsEvent::AgentCompleted {
                    agent_id: agent_id.clone(),
                    outcome_type: "blocked".to_string(),
                    summary: reason,
                    tokens_used: None,
                }),
                WorkerMessage::NodeCreated {
                    agent_id: _,
                    parent_id,
                    node,
                } => Some(WsEvent::NodeCreated {
                    node,
                    parent_id: Some(parent_id),
                }),
                WorkerMessage::NeedsDecision {
                    agent_id: _,
                    task_id: _,
                    decision,
                } => Some(WsEvent::NodeCreated {
                    node: decision,
                    parent_id: None,
                }),
                WorkerMessage::Cancel { .. } | WorkerMessage::AdditionalContext { .. } => None,
                WorkerMessage::ReviewRequest { .. } | WorkerMessage::ReviewFeedback { .. } => None,
            };

            if let Some(event) = event {
                let _ = ws_tx.send(event);
            }
        }
        tracing::debug!("WS bridge task ended");
    })
}
