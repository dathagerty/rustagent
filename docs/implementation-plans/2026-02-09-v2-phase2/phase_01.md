# Rustagent V2 Phase 2a: Message Bus

**Goal:** Implement the in-memory message bus for real-time orchestrator-worker communication, including the WorkerMessage enum and TokioMessageBus using broadcast + per-agent mpsc channels.

**Architecture:** Two-channel communication model — SQLite is the source of truth for all durable state; the message bus provides best-effort, fire-and-forget notifications to reduce polling latency. The bus has no durability guarantees. If any message is lost, correctness is preserved because the orchestrator discovers the same information by querying the DB on its next scheduling pass.

**Tech Stack:** Rust (edition 2024), tokio 1.43, tokio-util 0.7 (new dependency for CancellationToken), async-trait 0.1

**Scope:** Phase 1 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2a.AC1: WorkerMessage types
- **P2a.AC1.1 Success:** WorkerMessage enum has all 9 variants: ProgressReport, TaskCompleted, TaskBlocked, NeedsDecision, NodeCreated, Cancel, AdditionalContext, ReviewRequest, ReviewFeedback
- **P2a.AC1.2 Success:** Each variant carries the correct fields as specified in the architecture
- **P2a.AC1.3 Success:** WorkerMessage is Clone + Debug + Send + Sync

### P2a.AC2: MessageBus trait
- **P2a.AC2.1 Success:** MessageBus trait defines `send()`, `broadcast()`, and `subscribe()` methods
- **P2a.AC2.2 Success:** `send()` delivers a message to a specific agent's mpsc channel
- **P2a.AC2.3 Success:** `broadcast()` delivers a message to all subscribed agents via the broadcast channel
- **P2a.AC2.4 Success:** `subscribe()` returns a receiver that gets both targeted and broadcast messages

### P2a.AC3: TokioMessageBus implementation
- **P2a.AC3.1 Success:** TokioMessageBus uses `tokio::sync::broadcast` for fan-out messages and per-agent `tokio::sync::mpsc` for targeted messages
- **P2a.AC3.2 Success:** Multiple agents can subscribe and each receives broadcast messages
- **P2a.AC3.3 Success:** Targeted messages reach only the intended agent
- **P2a.AC3.4 Success:** Messages sent before subscription are not required to be received (fire-and-forget semantics)
- **P2a.AC3.5 Success:** Dropping a subscriber does not crash the bus (lagged receivers are handled gracefully)

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Add tokio-util dependency

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/Cargo.toml`

**Implementation:**

Add `tokio-util` to `[dependencies]` after the existing `tokio-rusqlite` entry:

```toml
tokio-util = "0.7"
```

This provides `CancellationToken` (used by WorkerHandle in later phases) and other tokio utilities. Adding it now because the message module will be the foundation for the orchestrator.

**Verification:**

Run: `cargo check`
Expected: Compiles without errors

**Commit:** `chore: add tokio-util dependency for orchestration`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create message module with WorkerMessage and TokioMessageBus

**Verifies:** P2a.AC1.1, P2a.AC1.2, P2a.AC1.3, P2a.AC2.1, P2a.AC2.2, P2a.AC2.3, P2a.AC2.4, P2a.AC3.1, P2a.AC3.2, P2a.AC3.3, P2a.AC3.4, P2a.AC3.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/message.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod message;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/message_test.rs` (integration)

**Design note:** The architecture specifies `src/message/` as a directory with `mod.rs`, `bus.rs`, `envelope.rs` and an `Envelope`/`AgentMessage` type. This phase implements a flat `src/message.rs` file because the Envelope type (wrapping messages with sender/recipient/timestamp metadata) adds complexity without clear benefit at this stage — the sender is already embedded in Worker→Orchestrator variants via `agent_id`, and the MessageBus trait handles routing. If the Envelope type becomes needed (e.g., for message logging, replay, or ordered delivery), it can be extracted into a `src/message/` directory in a future phase without breaking the public API (`MessageBus` trait and `WorkerMessage` enum remain the same).

**Implementation:**

`src/message.rs` contains three main items:

**1. WorkerMessage enum** — all 9 variants from the architecture:

```rust
use crate::agent::AgentId;
use crate::graph::GraphNode;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::sync::mpsc;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    // Worker → Orchestrator
    ProgressReport { agent_id: AgentId, turn: usize, summary: String },
    TaskCompleted { agent_id: AgentId, task_id: String, summary: String },
    TaskBlocked { agent_id: AgentId, task_id: String, reason: String },
    NeedsDecision { agent_id: AgentId, task_id: String, decision: GraphNode },
    NodeCreated { agent_id: AgentId, parent_id: String, node: GraphNode },

    // Orchestrator → Worker
    Cancel { reason: String },
    AdditionalContext { content: String },

    // Worker ↔ Worker (review flow)
    ReviewRequest { work_package_id: String, changed_files: Vec<PathBuf> },
    ReviewFeedback { approved: bool, comments: Vec<String> },
}
```

Note: Each Worker→Orchestrator variant includes `agent_id` so the orchestrator can identify the sender without relying on channel metadata. The architecture's `changed_files` uses `Vec<String>` but `Vec<PathBuf>` is more idiomatic for file paths in Rust.

**2. MessageBus trait:**

```rust
#[async_trait]
pub trait MessageBus: Send + Sync {
    /// Send a message to a specific agent
    async fn send(&self, to: &AgentId, msg: WorkerMessage) -> Result<()>;

    /// Broadcast a message to all subscribers
    async fn broadcast(&self, msg: WorkerMessage) -> Result<()>;

    /// Create a subscription for an agent. Returns a receiver that gets
    /// both targeted messages (via send) and broadcast messages.
    fn subscribe(&self, agent_id: &AgentId) -> mpsc::Receiver<WorkerMessage>;
}
```

**3. TokioMessageBus struct:**

Architecture pattern: broadcast channel for fan-out + per-agent mpsc for targeted delivery.

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;

pub struct TokioMessageBus {
    broadcast_tx: broadcast::Sender<WorkerMessage>,
    /// Per-agent mpsc senders. Protected by Mutex because subscribe/send are
    /// called from different tasks but never held across await points.
    agent_channels: Mutex<HashMap<AgentId, mpsc::Sender<WorkerMessage>>>,
    /// Channel capacity for per-agent mpsc channels
    agent_channel_capacity: usize,
}
```

- `new(broadcast_capacity: usize, agent_channel_capacity: usize) -> Self` — creates broadcast channel, empty agent map. Reasonable defaults: broadcast_capacity=64, agent_channel_capacity=32.
- `subscribe()` — creates a new mpsc channel pair, stores the sender in agent_channels, spawns a background tokio task that reads from `broadcast_tx.subscribe()` and forwards to the mpsc sender (so the consumer gets one unified receiver). Returns the mpsc receiver.
- `send()` — looks up the agent's mpsc sender and sends the message. Returns error if agent not subscribed (log warning but don't panic — fire-and-forget semantics).
- `broadcast()` — sends on the broadcast channel. Ignores `SendError` (no subscribers) gracefully.

The subscribe forwarding task should handle `broadcast::error::RecvError::Lagged` by logging a warning and continuing (not crashing). When the mpsc receiver is dropped, the forwarding task should detect the send failure and exit cleanly.

Also implement a `remove_subscriber(&self, agent_id: &AgentId)` method to clean up the agent_channels entry when a worker completes. This prevents memory leaks from accumulated dead channels.

**Testing:**

Tests in `tests/message_test.rs` using `#[tokio::test]`:

- P2a.AC1.1: Construct each of the 9 WorkerMessage variants — verifies the enum compiles with correct field names
- P2a.AC1.3: Assert WorkerMessage is Clone + Debug (clone a message, format with Debug)
- P2a.AC2.2: Create TokioMessageBus, subscribe agent "a1", send a message to "a1", receive it on the subscriber
- P2a.AC2.3: Subscribe two agents, broadcast a message, both receive it
- P2a.AC3.3: Subscribe two agents, send targeted message to "a1", verify "a1" receives it and "a2" does not (use `tokio::time::timeout` to confirm "a2" gets nothing)
- P2a.AC3.4: Broadcast a message, then subscribe — subscriber should not receive the earlier message (verify with timeout)
- P2a.AC3.5: Subscribe an agent, drop the receiver, broadcast — bus should not panic
- remove_subscriber: Subscribe agent "a1", remove it, send to "a1" — returns error (agent not found)

Follow project testing patterns: `#[tokio::test]`, `assert_eq!`/`assert!` for assertions, `anyhow::Result<()>` return types where convenient.

**Verification:**

Run: `cargo test message_test`
Expected: All tests pass

**Commit:** `feat(message): WorkerMessage types, MessageBus trait, and TokioMessageBus implementation`

<!-- END_TASK_2 -->
<!-- END_SUBCOMPONENT_A -->
