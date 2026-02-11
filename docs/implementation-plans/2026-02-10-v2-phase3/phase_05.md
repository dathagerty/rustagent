# Rustagent V2 Phase 3e: WebSocket Event Streaming

**Goal:** Implement real-time event streaming from the daemon to clients via WebSocket. Events are bridged from the internal MessageBus (WorkerMessage) and graph state changes to WebSocket subscribers.

**Architecture:** The daemon exposes `WS /ws` for real-time event streaming. A `WsBroadcaster` bridges internal events (WorkerMessage from the MessageBus, graph mutations) to a `broadcast::Sender<WsEvent>`. WebSocket clients subscribe and receive JSON-encoded events. The connection handles client disconnects gracefully.

**Tech Stack:** Rust (edition 2024), axum 0.8 (ws feature), tokio 1.43, serde_json

**Scope:** Phase 5 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3e.AC1: WsEvent types
- **P3e.AC1.1 Success:** `WsEvent` enum has all 9 variants from the architecture: `agent_spawned`, `agent_progress`, `agent_completed`, `node_created`, `node_status_changed`, `edge_created`, `session_ended`, `tool_execution`, `orchestrator_state_changed`
- **P3e.AC1.2 Success:** Each variant carries the correct fields per the architecture spec
- **P3e.AC1.3 Success:** WsEvent serializes to JSON with a `type` field for variant discrimination (serde `tag = "type"`)

### P3e.AC2: WebSocket handler
- **P3e.AC2.1 Success:** `WS /ws` upgrades HTTP connection to WebSocket
- **P3e.AC2.2 Success:** Connected clients receive all WsEvent broadcasts as JSON text messages
- **P3e.AC2.3 Success:** Multiple simultaneous WebSocket connections each receive all events
- **P3e.AC2.4 Success:** Client disconnect does not cause errors on the broadcast side
- **P3e.AC2.5 Success:** WebSocket handler sends a heartbeat/ping at a configurable interval to detect stale connections

### P3e.AC3: MessageBus-to-WsEvent bridge
- **P3e.AC3.1 Success:** `WsBroadcaster` subscribes to the MessageBus and maps `WorkerMessage` variants to `WsEvent` variants
- **P3e.AC3.2 Success:** `WorkerMessage::ProgressReport` maps to `WsEvent::AgentProgress`
- **P3e.AC3.3 Success:** `WorkerMessage::TaskCompleted` maps to `WsEvent::AgentCompleted` with `outcome_type: "completed"`
- **P3e.AC3.4 Success:** `WorkerMessage::NodeCreated` maps to `WsEvent::NodeCreated` with full `GraphNode`
- **P3e.AC3.5 Success:** `WorkerMessage::TaskBlocked` maps to `WsEvent::AgentCompleted` with `outcome_type: "blocked"`

### P3e.AC4: Deferred event emission (documented)
- **P3e.AC4.1 Info:** `WsEvent::NodeStatusChanged` emission is deferred — requires hooks in `GraphStore::update_node` or API handlers that emit after successful mutation. Will be wired when orchestrator integrates with the daemon.
- **P3e.AC4.2 Info:** `WsEvent::EdgeCreated` emission is deferred — same rationale.
- **P3e.AC4.3 Info:** `WsEvent::SessionEnded` emission is deferred — requires orchestrator to emit directly via `ws_tx`.
- **P3e.AC4.4 Info:** `WsEvent::ToolExecution` emission is deferred — requires `AgentRuntime` to emit tool calls. No `WorkerMessage::ToolExecution` variant exists; this would need to be added in a future phase.
- **P3e.AC4.5 Info:** `WsEvent::OrchestratorStateChanged` emission is deferred — requires orchestrator state machine to emit on transitions via `ws_tx`.

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Define WsEvent enum with all variants

**Verifies:** P3e.AC1.1, P3e.AC1.2, P3e.AC1.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs` — replace WsEvent placeholder with full definition
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_ws_test.rs`

**Implementation:**

Replace the placeholder `WsEvent` in `src/daemon/api/mod.rs`:

```rust
use serde::Serialize;

/// WebSocket event types matching the architecture specification.
///
/// Emission sources:
/// - AgentSpawned, AgentProgress, AgentCompleted: Bridged from WorkerMessage via MessageBus
/// - NodeCreated: Bridged from WorkerMessage::NodeCreated via MessageBus
/// - NodeStatusChanged: Emitted by graph mutation hooks (deferred — see note below)
/// - EdgeCreated: Emitted by graph mutation hooks (deferred — see note below)
/// - SessionEnded: Emitted directly by orchestrator via ws_tx (deferred)
/// - ToolExecution: Emitted by AgentRuntime tool execution loop (deferred)
/// - OrchestratorStateChanged: Emitted directly by orchestrator via ws_tx (deferred)
///
/// NOTE: In Phase 3, only events bridged from the MessageBus are emitted (the first 4).
/// The remaining 5 events require hooks in the GraphStore, AgentRuntime, and Orchestrator
/// that will be wired when those components are integrated with the daemon. The event
/// types are defined now so the WsEvent enum is complete and WebSocket clients can
/// subscribe to the full event stream from the start.
#[derive(Debug, Clone, Serialize)]
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
    /// Architecture specifies `outcome: AgentOutcome`. We carry the outcome type as a
    /// string discriminant plus summary to keep WsEvent self-contained (no dependency
    /// on agent module types). Outcome types: "completed", "blocked", "failed", "budget_exhausted".
    AgentCompleted {
        agent_id: String,
        outcome_type: String,
        summary: String,
        tokens_used: Option<usize>,
    },
    /// Carries the full GraphNode for maximum client utility.
    /// GraphNode already derives Serialize.
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
    /// Carries the full GraphEdge for maximum client utility.
    /// GraphEdge already derives Serialize.
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
```

**Testing:**

Tests in `tests/daemon_ws_test.rs`:

- P3e.AC1.1: Construct each of the 9 WsEvent variants. Verifies the enum compiles with correct field names.
- P3e.AC1.3: Serialize each variant to JSON. Verify each has a `"type"` field with the snake_case variant name (e.g., `"type": "agent_spawned"`).

```rust
#[test]
fn test_ws_event_serialization() {
    let event = WsEvent::AgentSpawned {
        agent_id: "w-1".into(),
        profile: "coder".into(),
        goal_id: "ra-a3f8".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "agent_spawned");
    assert_eq!(json["agent_id"], "w-1");
}

#[test]
fn test_ws_event_agent_completed_serialization() {
    let event = WsEvent::AgentCompleted {
        agent_id: "w-1".into(),
        outcome_type: "blocked".into(),
        summary: "Missing dependency".into(),
        tokens_used: None,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "agent_completed");
    assert_eq!(json["outcome_type"], "blocked");
}
```

**Verification:**

Run: `cargo test daemon_ws_test`
Expected: All tests pass

**Commit:** `feat(daemon): WsEvent enum with all 9 event variants`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create WebSocket handler

**Verifies:** P3e.AC2.1, P3e.AC2.2, P3e.AC2.3, P3e.AC2.4, P3e.AC2.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/ws.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` — uncomment `pub mod ws;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount WS route
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_ws_test.rs`

**Implementation:**

`src/daemon/ws.rs`:

```rust
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use tokio::sync::broadcast;
use crate::daemon::api::{AppState, WsEvent};

/// WS /ws — WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.ws_tx.subscribe()))
}

/// Handle an individual WebSocket connection
async fn handle_socket(
    mut socket: WebSocket,
    mut rx: broadcast::Receiver<WsEvent>,
) {
    // Spawn a task to receive events and forward them to the WebSocket
    loop {
        tokio::select! {
            // Forward broadcast events to the WebSocket client
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
                            // Client disconnected
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
            // Handle incoming messages from client (for future use, e.g., subscribe to specific events)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    _ => {} // Ignore other messages for now
                }
            }
        }
    }

    tracing::debug!("WebSocket client disconnected");
}
```

**Route mounting** in `server.rs`:

```rust
use crate::daemon::ws;

// Add to create_router():
.route("/ws", get(ws::ws_handler))
```

**Testing:**

WebSocket testing requires a running server. Use `tokio::net::TcpListener` with a random port:

```rust
#[tokio::test]
async fn test_ws_receives_events() {
    let state = create_test_state().await;
    let ws_tx = state.ws_tx.clone();
    let router = rustagent::daemon::server::create_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Connect WebSocket client using tokio-tungstenite (add as dev-dependency)
    // Or use reqwest's WebSocket support
    // Send a WsEvent through ws_tx, verify client receives it
}
```

Note: Add `tokio-tungstenite` as a dev-dependency for WebSocket client testing:

```toml
[dev-dependencies]
tokio-tungstenite = "0.26"
```

Tests:

- P3e.AC2.1: Connect to `ws://localhost:{port}/ws`, verify upgrade succeeds
- P3e.AC2.2: Send WsEvent through `ws_tx`, verify client receives JSON message with correct `type` field
- P3e.AC2.3: Connect two clients, send event, both receive it
- P3e.AC2.4: Connect client, disconnect it, send event — no errors on the server side

**Verification:**

Run: `cargo test daemon_ws_test`
Expected: All tests pass

**Commit:** `feat(daemon): WebSocket handler for real-time event streaming`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Bridge MessageBus to WsEvent broadcaster

**Verifies:** P3e.AC3.1, P3e.AC3.2, P3e.AC3.3, P3e.AC3.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/ws.rs` — add WsBroadcaster
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` or `server.rs` — start the bridge on daemon startup
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_ws_test.rs`

**Implementation:**

Add to `ws.rs`:

```rust
use crate::message::{MessageBus, WorkerMessage};
use crate::agent::AgentId;
use std::sync::Arc;

/// Bridges WorkerMessage events from the MessageBus to WsEvent broadcasts.
/// Spawns a background task that subscribes to the MessageBus and maps
/// messages to WsEvents.
pub fn start_ws_bridge(
    message_bus: Arc<dyn MessageBus>,
    ws_tx: broadcast::Sender<WsEvent>,
) -> tokio::task::JoinHandle<()> {
    // Subscribe to the message bus as a special "ws-bridge" agent
    let bridge_id = AgentId("ws-bridge".to_string());
    let mut rx = message_bus.subscribe(&bridge_id);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let event = match msg {
                WorkerMessage::ProgressReport { agent_id, turn, summary } => {
                    Some(WsEvent::AgentProgress {
                        agent_id: agent_id.0,
                        turn,
                        summary,
                    })
                }
                WorkerMessage::TaskCompleted { agent_id, task_id: _, summary } => {
                    Some(WsEvent::AgentCompleted {
                        agent_id: agent_id.0,
                        outcome_type: "completed".to_string(),
                        summary,
                        tokens_used: None, // Token count not available from WorkerMessage
                    })
                }
                WorkerMessage::TaskBlocked { agent_id, task_id: _, reason } => {
                    Some(WsEvent::AgentCompleted {
                        agent_id: agent_id.0,
                        outcome_type: "blocked".to_string(),
                        summary: reason,
                        tokens_used: None,
                    })
                }
                WorkerMessage::NodeCreated { agent_id: _, parent_id, node } => {
                    Some(WsEvent::NodeCreated {
                        node,
                        parent_id: Some(parent_id),
                    })
                }
                WorkerMessage::NeedsDecision { agent_id: _, task_id: _, decision } => {
                    Some(WsEvent::NodeCreated {
                        node: decision,
                        parent_id: None,
                    })
                }
                // Orchestrator→Worker messages are not forwarded to WS clients
                WorkerMessage::Cancel { .. } | WorkerMessage::AdditionalContext { .. } => None,
                // Review flow messages
                WorkerMessage::ReviewRequest { .. } | WorkerMessage::ReviewFeedback { .. } => None,
            };

            if let Some(event) = event {
                // Ignore send errors (no subscribers)
                let _ = ws_tx.send(event);
            }
        }
        tracing::debug!("WS bridge task ended");
    })
}
```

In the daemon startup (within `DaemonAction::Start` in `main.rs` or in `server.rs`), start the bridge:

```rust
// After creating AppState:
let _ws_bridge = rustagent::daemon::ws::start_ws_bridge(
    state.message_bus.clone(),
    state.ws_tx.clone(),
);
```

**Testing:**

- P3e.AC3.1: Create MessageBus and WsBroadcaster. Subscribe to ws_tx. Send a WorkerMessage through the bus. Verify corresponding WsEvent is received.
- P3e.AC3.2: Send `ProgressReport` through MessageBus. Receive `AgentProgress` on ws_tx subscriber.
- P3e.AC3.3: Send `TaskCompleted` through MessageBus. Receive `AgentCompleted` with `success: true`.
- P3e.AC3.4: Send `NodeCreated` through MessageBus. Receive `NodeCreated` WsEvent.

**Verification:**

Run: `cargo test daemon_ws_test`
Expected: All tests pass

**Commit:** `feat(daemon): MessageBus to WebSocket event bridge`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
