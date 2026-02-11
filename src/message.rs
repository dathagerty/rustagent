use crate::agent::AgentId;
use crate::graph::GraphNode;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::sync::{broadcast, mpsc};
use tracing::warn;

/// Messages exchanged between workers and the orchestrator.
///
/// Worker->Orchestrator variants include `agent_id` so the orchestrator
/// can identify the sender without relying on channel metadata.
#[derive(Debug, Clone)]
pub enum WorkerMessage {
    // Worker -> Orchestrator
    ProgressReport {
        agent_id: AgentId,
        turn: usize,
        summary: String,
    },
    TaskCompleted {
        agent_id: AgentId,
        task_id: String,
        summary: String,
    },
    TaskBlocked {
        agent_id: AgentId,
        task_id: String,
        reason: String,
    },
    NeedsDecision {
        agent_id: AgentId,
        task_id: String,
        decision: GraphNode,
    },
    NodeCreated {
        agent_id: AgentId,
        parent_id: String,
        node: GraphNode,
    },

    // Orchestrator -> Worker
    Cancel {
        reason: String,
    },
    AdditionalContext {
        content: String,
    },

    // Worker <-> Worker (review flow)
    ReviewRequest {
        work_package_id: String,
        changed_files: Vec<PathBuf>,
    },
    ReviewFeedback {
        approved: bool,
        comments: Vec<String>,
    },
}

/// Trait for the in-memory message bus.
///
/// The bus provides best-effort, fire-and-forget delivery. If any message
/// is lost, correctness is preserved because the orchestrator discovers
/// the same information by querying the DB on its next scheduling pass.
#[async_trait]
pub trait MessageBus: Send + Sync {
    /// Send a message to a specific agent's channel.
    async fn send(&self, to: &AgentId, msg: WorkerMessage) -> Result<()>;

    /// Broadcast a message to all subscribers.
    async fn broadcast(&self, msg: WorkerMessage) -> Result<()>;

    /// Create a subscription for an agent. Returns a receiver that gets
    /// both targeted messages (via send) and broadcast messages.
    fn subscribe(&self, agent_id: &AgentId) -> mpsc::Receiver<WorkerMessage>;

    /// Remove a subscriber, cleaning up resources.
    fn remove_subscriber(&self, agent_id: &AgentId);
}

/// In-memory message bus using tokio broadcast + per-agent mpsc channels.
///
/// Architecture: broadcast channel for fan-out + per-agent mpsc for targeted
/// delivery. Each subscriber gets a unified mpsc receiver that aggregates
/// both targeted sends and forwarded broadcast messages.
pub struct TokioMessageBus {
    broadcast_tx: broadcast::Sender<WorkerMessage>,
    /// Per-agent mpsc senders. Protected by Mutex because subscribe/send are
    /// called from different tasks but never held across await points.
    agent_channels: Mutex<HashMap<AgentId, mpsc::Sender<WorkerMessage>>>,
    /// Channel capacity for per-agent mpsc channels.
    agent_channel_capacity: usize,
}

impl TokioMessageBus {
    /// Create a new TokioMessageBus.
    ///
    /// `broadcast_capacity` — buffer size for the broadcast channel (default: 64).
    /// `agent_channel_capacity` — buffer size for per-agent mpsc channels (default: 32).
    pub fn new(broadcast_capacity: usize, agent_channel_capacity: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(broadcast_capacity);
        Self {
            broadcast_tx,
            agent_channels: Mutex::new(HashMap::new()),
            agent_channel_capacity,
        }
    }
}

impl Default for TokioMessageBus {
    fn default() -> Self {
        Self::new(64, 32)
    }
}

#[async_trait]
impl MessageBus for TokioMessageBus {
    async fn send(&self, to: &AgentId, msg: WorkerMessage) -> Result<()> {
        let sender = {
            let channels = self.agent_channels.lock().map_err(|e| anyhow!("lock poisoned: {}", e))?;
            channels.get(to).cloned()
        };
        match sender {
            Some(tx) => {
                if tx.send(msg).await.is_err() {
                    warn!(agent_id = %to, "agent channel closed, message dropped");
                }
                Ok(())
            }
            None => Err(anyhow!("agent {} is not subscribed", to)),
        }
    }

    async fn broadcast(&self, msg: WorkerMessage) -> Result<()> {
        // Ignore SendError — means no active receivers, which is fine
        let _ = self.broadcast_tx.send(msg);
        Ok(())
    }

    fn subscribe(&self, agent_id: &AgentId) -> mpsc::Receiver<WorkerMessage> {
        let (tx, rx) = mpsc::channel(self.agent_channel_capacity);

        // Store the sender for targeted messages
        {
            let mut channels = self.agent_channels.lock().expect("lock poisoned");
            channels.insert(agent_id.clone(), tx.clone());
        }

        // Spawn a forwarding task: broadcast -> agent's mpsc
        let mut broadcast_rx = self.broadcast_tx.subscribe();
        tokio::spawn(async move {
            loop {
                match broadcast_rx.recv().await {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            // Receiver dropped — exit forwarding loop
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(lagged = n, "broadcast receiver lagged, skipping messages");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        rx
    }

    fn remove_subscriber(&self, agent_id: &AgentId) {
        let mut channels = self.agent_channels.lock().expect("lock poisoned");
        channels.remove(agent_id);
    }
}
