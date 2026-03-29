# Rustagent V2 Phase 2e: Agent Tools (spawn_sub_agent, send_message, query_agent_status)

**Goal:** Implement the agent-facing tools that allow workers to interact with the orchestration system: requesting sub-agent spawning, sending messages to other workers, and querying the status of other agents. These tools follow the existing `Tool` trait pattern and are registered alongside graph tools.

**Architecture:** Workers interact with the orchestrator through the message bus. The `spawn_sub_agent` tool doesn't directly spawn — it creates a task node in the graph and sends a message to the orchestrator requesting a new worker. The `send_message` tool uses the message bus for worker-to-worker communication (primarily the review flow). The `query_agent_status` tool reads from the graph store.

**Tech Stack:** Rust (edition 2024), tokio 1.43, async-trait 0.1, serde_json

**Scope:** Phase 5 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2e.AC1: spawn_sub_agent tool
- **P2e.AC1.1 Success:** Tool creates a new task node under the current goal as a child of the calling worker's task
- **P2e.AC1.2 Success:** Tool sends a NodeCreated message to the orchestrator via the message bus
- **P2e.AC1.3 Success:** Tool returns the created task node ID so the worker can track it
- **P2e.AC1.4 Success:** The new task is created with status Ready so the orchestrator picks it up in the next scheduling pass

### P2e.AC2: send_message tool
- **P2e.AC2.1 Success:** Tool accepts a target agent ID and message content, sends via message bus
- **P2e.AC2.2 Success:** Tool supports ReviewRequest and ReviewFeedback message types
- **P2e.AC2.3 Failure:** Sending to a non-existent agent returns a clear error message (not a crash)

### P2e.AC3: query_agent_status tool
- **P2e.AC3.1 Success:** Tool returns the current status of tasks assigned to a given agent
- **P2e.AC3.2 Success:** Tool returns "no agent found" when querying a non-existent agent

### P2e.AC4: Tool registration
- **P2e.AC4.1 Success:** All three tools are registered in the v2 registry
- **P2e.AC4.2 Success:** Tools implement the existing Tool trait (name, description, parameters, execute)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create agent_tools module with SpawnSubAgentTool

**Verifies:** P2e.AC1.1, P2e.AC1.2, P2e.AC1.3, P2e.AC1.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/agent_tools.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/mod.rs` — add `pub mod agent_tools;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/agent_tools_test.rs` (integration)

**Implementation:**

`src/tools/agent_tools.rs`:

**SpawnSubAgentTool:**

```rust
use crate::agent::AgentId;
use crate::graph::store::GraphStore;
use crate::graph::{GraphNode, NodeType, NodeStatus, generate_child_id, generate_edge_id, GraphEdge, EdgeType};
use crate::message::{MessageBus, WorkerMessage};
use crate::tools::Tool;
use async_trait::async_trait;
use std::sync::Arc;

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
        Self { graph_store, message_bus, agent_id }
    }
}
```

The tool accepts JSON parameters:
- `title` (required): Title for the new task
- `description` (required): What the sub-agent should do
- `parent_task_id` (required): The calling worker's task ID (used to generate child ID)
- `profile` (optional, defaults to "coder"): Which agent profile the new worker should use
- `file_scope` (optional): Files the sub-agent will need to modify

On execute:
1. Get the next child sequence number by querying existing children of parent_task_id
2. Generate child ID via `generate_child_id(parent_task_id, seq)`
3. Create a new GraphNode with NodeType::Task, status Ready
4. Store profile and file_scope in the node's metadata
5. Insert the node via graph_store.create_node()
6. Send NodeCreated message to orchestrator via message_bus.broadcast()
7. Return JSON: `{"task_id": "<new_id>", "status": "ready"}`

**Testing:**

Tests use Database::open_in_memory(), SqliteGraphStore, and TokioMessageBus:

- P2e.AC1.1: Call execute with valid params → new task node exists in graph store as child of parent
- P2e.AC1.3: Return value contains the new task_id
- P2e.AC1.4: New task node has status Ready

**Verification:**

Run: `cargo test agent_tools_test`
Expected: All tests pass

**Commit:** `feat(tools): SpawnSubAgentTool for worker-initiated task creation`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Implement SendMessageTool and QueryAgentStatusTool

**Verifies:** P2e.AC2.1, P2e.AC2.2, P2e.AC2.3, P2e.AC3.1, P2e.AC3.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/agent_tools.rs` — add SendMessageTool and QueryAgentStatusTool
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/agent_tools_test.rs` — add tests

**Implementation:**

**SendMessageTool:**

```rust
pub struct SendMessageTool {
    message_bus: Arc<dyn MessageBus>,
    agent_id: AgentId,
}
```

Parameters:
- `target_agent_id` (required): The agent to send the message to
- `message_type` (required): One of "review_request" or "review_feedback" or "additional_context"
- `content` (required): JSON content appropriate for the message type

For "review_request": content should include `work_package_id` and `changed_files`
For "review_feedback": content should include `approved` (bool) and `comments` (array)
For "additional_context": content should include the context string

The tool constructs the appropriate WorkerMessage variant and calls `message_bus.send()`.

If the send fails (e.g., no such agent), return a descriptive error string (not an Err — the LLM should see the error and self-correct).

**QueryAgentStatusTool:**

```rust
pub struct QueryAgentStatusTool {
    graph_store: Arc<dyn GraphStore>,
}
```

Parameters:
- `agent_id` (required): The agent whose task status to query

Queries the graph store for task nodes where `assigned_to == agent_id`. Returns a JSON summary:
```json
{
    "agent_id": "worker-1",
    "tasks": [
        {"task_id": "ra-a3f8.1", "status": "in_progress", "title": "Implement auth"},
        {"task_id": "ra-a3f8.2", "status": "completed", "title": "Add tests"}
    ]
}
```

If no tasks found for the agent, returns `{"agent_id": "...", "tasks": [], "note": "no tasks found for this agent"}`.

**Testing:**

- P2e.AC2.1: Create SendMessageTool, send a message to a subscribed agent → message received
- P2e.AC2.3: Send to non-existent agent → returns error string (not panic)
- P2e.AC3.1: Create tasks assigned to "worker-1", query → returns correct status
- P2e.AC3.2: Query for non-existent agent → returns empty tasks array

**Verification:**

Run: `cargo test agent_tools_test`
Expected: All tests pass

**Commit:** `feat(tools): SendMessageTool and QueryAgentStatusTool`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Register agent tools in v2 registry

**Verifies:** P2e.AC4.1, P2e.AC4.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/factory.rs` — update `create_v2_registry` signature and register agent tools
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/agent_tools_test.rs` — add registration test

**Implementation:**

The `create_v2_registry` function needs additional parameters to create the agent tools:

```rust
pub fn create_v2_registry(
    validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    graph_store: Arc<dyn GraphStore>,
    message_bus: Option<Arc<dyn MessageBus>>,  // None for single-agent mode
    agent_id: Option<AgentId>,                 // None for single-agent mode
) -> ToolRegistry {
    let registry = create_default_registry(validator, permission_handler);

    // ... existing graph tool registrations ...

    // Register agent tools (only in multi-agent mode)
    if let (Some(bus), Some(id)) = (message_bus, agent_id) {
        registry.register(Arc::new(SpawnSubAgentTool::new(
            graph_store.clone(), bus.clone(), id.clone()
        )));
        registry.register(Arc::new(SendMessageTool::new(bus.clone(), id.clone())));
        registry.register(Arc::new(QueryAgentStatusTool::new(graph_store.clone())));
    }

    registry
}
```

Update all existing call sites of `create_v2_registry` in `src/main.rs` to pass `None, None` for the new parameters (single-agent mode doesn't need agent tools).

**Testing:**

- P2e.AC4.1: Create v2 registry with message_bus and agent_id → registry contains "spawn_sub_agent", "send_message", "query_agent_status"
- P2e.AC4.2: Create v2 registry without message_bus (None) → registry does not contain agent tools (backward compat)

**Verification:**

Run: `cargo test agent_tools_test`
Run: `cargo test` (full suite to verify no regressions from registry signature change)
Expected: All tests pass

**Commit:** `feat(tools): register agent tools in v2 registry for multi-agent mode`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
