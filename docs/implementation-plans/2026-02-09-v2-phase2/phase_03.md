# Rustagent V2 Phase 2c: Orchestrator Core State Machine

**Goal:** Build the Orchestrator struct with its deterministic state machine (Startup → Loading → Planning → Scheduling → Monitoring → Reviewing → Completing), OrchestratorConfig, and OrchestratorState enum. This phase focuses on the state machine skeleton and state transition logic; worker spawning and message handling are wired up in Phase 2d.

**Architecture:** The orchestrator is NOT an LLM agent. It's a deterministic state machine that coordinates work — it doesn't burn tokens on coordination logic, it follows rules. The orchestrator holds references to the GraphStore, MessageBus, active workers map, and file ownership map. State transitions are driven by DB queries and worker message events.

**Tech Stack:** Rust (edition 2024), tokio 1.43, tokio-util 0.7, async-trait 0.1, chrono

**Scope:** Phase 3 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2c.AC1: OrchestratorConfig
- **P2c.AC1.1 Success:** OrchestratorConfig has all fields from the architecture: max_concurrent_workers, max_retries_per_task, worker_turn_limit, check_in_interval, review_required, max_consecutive_llm_failures, max_consecutive_tool_failures, worker_token_budget, token_budget_warning_pct, max_tokens_per_goal
- **P2c.AC1.2 Success:** Default values match architecture: max_concurrent_workers=4, max_retries_per_task=2, worker_turn_limit=100, check_in_interval=10, worker_token_budget=200_000, token_budget_warning_pct=80, max_consecutive_llm_failures=3, max_consecutive_tool_failures=3

**Implementation decision:** `check_in_interval` defaults to 10 (every 10 turns). The architecture specifies check-in intervals but does not prescribe a default. We chose 10 to provide progress visibility without excessive message bus traffic.

### P2c.AC2: OrchestratorState
- **P2c.AC2.1 Success:** OrchestratorState enum has 7 variants: Startup, Loading, Planning, Scheduling, Monitoring, Reviewing, Completing
- **P2c.AC2.2 Success:** Transitions follow the defined state machine graph (no invalid transitions)

### P2c.AC3: Orchestrator struct
- **P2c.AC3.1 Success:** Orchestrator holds config, graph_store, message_bus, active_workers, file_locks
- **P2c.AC3.2 Success:** `Orchestrator::new()` initializes in Startup state

### P2c.AC4: Recovery logic
- **P2c.AC4.1 Success:** On startup with an interrupted session, InProgress tasks are reset to Ready
- **P2c.AC4.2 Success:** Recovery resumes from Scheduling state after reset

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Create orchestrator module with OrchestratorConfig and OrchestratorState

**Verifies:** P2c.AC1.1, P2c.AC1.2, P2c.AC2.1

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/mod.rs` — add `pub mod orchestrator;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` (integration)

**Implementation:**

`src/agent/orchestrator.rs`:

**OrchestratorConfig:**

```rust
/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of concurrent workers (default: 4)
    pub max_concurrent_workers: usize,
    /// Maximum retries per failed task (default: 2)
    pub max_retries_per_task: usize,
    /// Maximum turns per worker (default: 100)
    pub worker_turn_limit: usize,
    /// Worker progress report interval in turns
    pub check_in_interval: usize,
    /// Whether to spawn a reviewer after each coder completes
    pub review_required: bool,
    /// Max consecutive LLM failures before blocking a worker (default: 3)
    pub max_consecutive_llm_failures: usize,
    /// Max consecutive tool failures before blocking a worker (default: 3)
    pub max_consecutive_tool_failures: usize,
    /// Per-worker token budget (default: 200_000)
    pub worker_token_budget: usize,
    /// Token budget warning threshold as percentage (default: 80)
    pub token_budget_warning_pct: u8,
    /// Optional goal-level token budget (pause + approval if exceeded)
    pub max_tokens_per_goal: Option<usize>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workers: 4,
            max_retries_per_task: 2,
            worker_turn_limit: 100,
            check_in_interval: 10,
            review_required: false,
            max_consecutive_llm_failures: 3,
            max_consecutive_tool_failures: 3,
            worker_token_budget: 200_000,
            token_budget_warning_pct: 80,
            max_tokens_per_goal: None,
        }
    }
}
```

**OrchestratorState:**

```rust
/// State of the orchestrator state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestratorState {
    /// Load config, connect DB, check for interrupted session
    Startup,
    /// Load or create goal from user input
    Loading,
    /// Spawn planner worker to create initial task breakdown
    Planning,
    /// Query ready tasks, group into work packages, spawn workers
    Scheduling,
    /// Wait for worker messages (progress, completion, blocks)
    Monitoring,
    /// Spawn reviewer workers if review_required
    Reviewing,
    /// Generate session summary, report results
    Completing,
}
```

**Testing:**

Tests in `tests/orchestrator_test.rs`:

- P2c.AC1.1: Construct OrchestratorConfig with all fields
- P2c.AC1.2: `OrchestratorConfig::default()` matches documented defaults (assert each field)
- P2c.AC2.1: Construct each OrchestratorState variant, format with Debug

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): OrchestratorConfig with defaults and OrchestratorState enum`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Implement Orchestrator struct with state machine skeleton

**Verifies:** P2c.AC2.2, P2c.AC3.1, P2c.AC3.2, P2c.AC4.1, P2c.AC4.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — add Orchestrator struct and run method
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**Orchestrator struct:**

```rust
use crate::agent::AgentId;
use crate::agent::work_package::{FileOwnershipMap, WorkerHandle};
use crate::graph::store::GraphStore;
use crate::message::MessageBus;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Orchestrator {
    config: OrchestratorConfig,
    state: OrchestratorState,
    graph_store: Arc<dyn GraphStore>,
    message_bus: Arc<dyn MessageBus>,
    active_workers: HashMap<AgentId, WorkerHandle>,
    file_locks: FileOwnershipMap,
    goal_id: Option<String>,
    session_id: Option<String>,
    cumulative_tokens: usize,
}
```

**Constructor:**

```rust
impl Orchestrator {
    pub fn new(
        config: OrchestratorConfig,
        graph_store: Arc<dyn GraphStore>,
        message_bus: Arc<dyn MessageBus>,
    ) -> Self {
        Self {
            config,
            state: OrchestratorState::Startup,
            graph_store,
            message_bus,
            active_workers: HashMap::new(),
            file_locks: FileOwnershipMap::new(),
            goal_id: None,
            session_id: None,
            cumulative_tokens: 0,
        }
    }

    /// Get the current state
    pub fn state(&self) -> &OrchestratorState { &self.state }
}
```

**State machine run method** — a loop that dispatches based on current state. Each state handler returns the next state or an error. This phase implements the skeleton with stub handlers for Planning/Monitoring/Reviewing:

```rust
impl Orchestrator {
    /// Run the orchestrator to completion for a given goal.
    pub async fn run(&mut self, goal_description: &str, project_id: &str) -> Result<()> {
        loop {
            match &self.state {
                OrchestratorState::Startup => {
                    self.state = self.handle_startup(project_id).await?;
                }
                OrchestratorState::Loading => {
                    self.state = self.handle_loading(goal_description, project_id).await?;
                }
                OrchestratorState::Planning => {
                    self.state = self.handle_planning().await?;
                }
                OrchestratorState::Scheduling => {
                    self.state = self.handle_scheduling().await?;
                }
                OrchestratorState::Monitoring => {
                    self.state = self.handle_monitoring().await?;
                }
                OrchestratorState::Reviewing => {
                    self.state = self.handle_reviewing().await?;
                }
                OrchestratorState::Completing => {
                    self.handle_completing().await?;
                    return Ok(());
                }
            }
        }
    }
}
```

**handle_startup** — check for interrupted session, recover if needed:

```rust
async fn handle_startup(&mut self, project_id: &str) -> Result<OrchestratorState> {
    // Check for an interrupted session by querying the latest session without completed_at
    // For now: just transition to Loading (recovery is below)
    Ok(OrchestratorState::Loading)
}
```

**handle_loading** — check if goal exists (possibly from previous session), create if not:

```rust
async fn handle_loading(&mut self, goal_description: &str, project_id: &str) -> Result<OrchestratorState> {
    // Query for an existing active goal node for this project
    // If found: set self.goal_id, check if tasks exist
    // If not found: create a new goal node
    // Transition to Planning if no tasks, Scheduling if tasks exist
    todo!("Full implementation in Phase 2d")
}
```

**handle_scheduling** — query ready tasks, group into packages, check capacity:

```rust
async fn handle_scheduling(&mut self) -> Result<OrchestratorState> {
    // 1. Query ready tasks from graph store
    // 2. If no ready tasks and no active workers → Completing (or Reviewing)
    // 3. Group ready tasks into work packages
    // 4. For each package (up to max_concurrent_workers):
    //    - Acquire file locks
    //    - Spawn worker
    // 5. Transition to Monitoring
    todo!("Full implementation in Phase 2d")
}
```

Stubs for Planning, Monitoring, Reviewing, Completing — all `todo!()` for Phase 2d.

**Recovery logic** — implement as a private method called from handle_startup:

```rust
async fn recover_interrupted_session(&mut self, project_id: &str) -> Result<Option<String>> {
    // 1. Find the latest session for this project that has no completed_at
    // 2. If found: load goal_id, reset InProgress tasks to Ready
    // 3. Return the goal_id (if any)
    // The task status reset uses graph_store.update_node() to set status back to Ready
    // for any task nodes that were InProgress
    todo!("Full implementation in Phase 2d")
}
```

The important thing this phase establishes is the **type structure and state transition logic**. Phase 2d will fill in the actual implementations.

**Testing:**

Tests in `tests/orchestrator_test.rs` using `#[tokio::test]`:

- P2c.AC3.2: `Orchestrator::new()` starts in Startup state
- P2c.AC3.1: Verify all fields are accessible (create with mock graph_store and mock message_bus from tests/common)

For the tests, create a MockMessageBus implementing the MessageBus trait:

```rust
use rustagent::message::{MessageBus, WorkerMessage};
use rustagent::agent::AgentId;
use async_trait::async_trait;
use tokio::sync::mpsc;

struct MockMessageBus;

#[async_trait]
impl MessageBus for MockMessageBus {
    async fn send(&self, _to: &AgentId, _msg: WorkerMessage) -> anyhow::Result<()> {
        Ok(())
    }
    async fn broadcast(&self, _msg: WorkerMessage) -> anyhow::Result<()> {
        Ok(())
    }
    fn subscribe(&self, _agent_id: &AgentId) -> mpsc::Receiver<WorkerMessage> {
        let (_tx, rx) = mpsc::channel(1);
        rx
    }
}
```

Use the existing `MockGraphStore` from `tests/common/mod.rs` if available, or `SqliteGraphStore` with `Database::open_in_memory()`.

- P2c.AC2.2: Verify state transitions — create orchestrator, check initial state is Startup, manually set state to each valid transition to confirm the enum works. (The `run()` method's actual transitions are tested in Phase 2d when handlers are implemented.)

Note: Recovery logic (P2c.AC4.1, P2c.AC4.2) tests will be added in Phase 2d when the handler implementations are complete. This phase verifies the struct compiles and the state enum is correct.

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): Orchestrator struct with state machine skeleton and recovery stubs`

<!-- END_TASK_2 -->
<!-- END_SUBCOMPONENT_A -->
