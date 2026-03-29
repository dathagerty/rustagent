# Rustagent V2 Phase 2d: Orchestrator-Runtime Integration

**Goal:** Wire up the orchestrator's state handlers to the AgentRuntime, implementing worker spawning, message handling, check-in intervals, turn limits, file scope enforcement, recovery logic, the full scheduling/monitoring/reviewing loop, built-in profile enhancement, worker conversation persistence, and end-to-end integration testing. This phase fills in the `todo!()` stubs from Phase 2c and makes the orchestrator functional.

**Architecture:** Workers are spawned as tokio tasks. Each worker gets an AgentContext with its work package, runs the standard agentic loop via AgentRuntime, and reports back through the MessageBus. The orchestrator monitors workers via message reception and JoinHandle completion, handles errors with retry semantics, and tracks cumulative token usage.

**Tech Stack:** Rust (edition 2024), tokio 1.43, tokio-util 0.7, async-trait 0.1, chrono

**Scope:** Phase 4 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2d.AC1: Worker spawning
- **P2d.AC1.1 Success:** Orchestrator spawns a worker as a tokio task for each work package
- **P2d.AC1.2 Success:** Each worker receives an AgentContext with correct work_package_tasks, project_path, profile, and graph_store
- **P2d.AC1.3 Success:** Worker is tracked via WorkerHandle in active_workers map
- **P2d.AC1.4 Success:** CancellationToken is created per worker and stored in WorkerHandle

### P2d.AC2: Message handling in Monitoring state
- **P2d.AC2.1 Success:** Orchestrator receives TaskCompleted messages and marks tasks Completed
- **P2d.AC2.2 Success:** Orchestrator receives TaskBlocked messages and marks tasks Blocked
- **P2d.AC2.3 Success:** Orchestrator receives ProgressReport messages and updates last_check_in
- **P2d.AC2.4 Success:** When a worker's JoinHandle completes, orchestrator processes the AgentOutcome

### P2d.AC3: Scheduling logic
- **P2d.AC3.1 Success:** Orchestrator queries ready tasks from graph store
- **P2d.AC3.2 Success:** Orchestrator groups ready tasks into work packages (using group_tasks_into_packages)
- **P2d.AC3.3 Success:** Orchestrator respects max_concurrent_workers limit
- **P2d.AC3.4 Success:** When no ready tasks remain and no workers are active, orchestrator transitions to Reviewing or Completing

### P2d.AC4: Error handling and retries
- **P2d.AC4.1 Success:** When a worker fails or returns Blocked, the task retry count is checked against max_retries_per_task
- **P2d.AC4.2 Success:** If retries remain, task status is reset to Ready for re-scheduling with fresh context
- **P2d.AC4.3 Success:** If retries exhausted, task is marked Failed and an Observation node documents the failure
- **P2d.AC4.4 Success:** Worker crash (JoinHandle error) is treated identically to a persistent failure

### P2d.AC5: Recovery logic
- **P2d.AC5.1 Success:** On startup with an interrupted session, InProgress tasks are reset to Ready
- **P2d.AC5.2 Success:** Recovery resumes from Scheduling state after reset

### P2d.AC6: Token accounting
- **P2d.AC6.1 Success:** Orchestrator tracks cumulative tokens across all workers for the goal
- **P2d.AC6.2 Success:** If max_tokens_per_goal is set and exceeded, orchestrator pauses and returns a budget-exceeded result
- **P2d.AC6.3 Success:** AgentOutcome::Completed variant includes `tokens_used: usize` field for reliable token propagation from worker to orchestrator

### P2d.AC7: Built-in profile enhancement
- **P2d.AC7.1 Success:** Built-in profiles use the structured system prompt template: role, task, acceptance criteria, context, rules sections
- **P2d.AC7.2 Success:** Each profile has profile-specific rules matching the design (planner: independent tasks + acceptance criteria; coder: check AC before completion + scoped writes + commit logical units; reviewer: no modifications + report as observations; tester: test behavior not implementation; researcher: document findings)

### P2d.AC8: Check-in interval support
- **P2d.AC8.1 Success:** AgentRuntime sends ProgressReport via MessageBus every `check_in_interval` turns
- **P2d.AC8.2 Success:** RuntimeConfig includes a reference to the message bus (optional, None for single-agent mode)

### P2d.AC9: Worker conversation persistence
- **P2d.AC9.1 Success:** On worker completion, the full conversation (messages, tool calls, tool results) is written to the worker_conversations table
- **P2d.AC9.2 Success:** worker_conversations record includes session_id, agent_id, task_ids, total_input_tokens, total_output_tokens

### P2d.AC10: File scope enforcement
- **P2d.AC10.1 Success:** SecurityValidator checks FileOwnershipMap.can_write() before allowing file write operations
- **P2d.AC10.2 Success:** A worker writing outside its declared file scope is blocked with a clear error message

### P2d.AC11: Reviewing state
- **P2d.AC11.1 Success:** handle_reviewing spawns a reviewer worker for completed work packages when review_required=true
- **P2d.AC11.2 Success:** After reviewer completes, orchestrator returns to Scheduling (to pick up any new tasks from review feedback) or Completing

### P2d.AC12: Dynamic adaptation
- **P2d.AC12.1 Success:** File scope expansion: When a worker signals NeedsDecision for files, orchestrator checks conflicts and either expands scope or re-queues the task
- **P2d.AC12.2 Success:** Task splitting: Worker-created subtask nodes are discovered by orchestrator in next scheduling pass

### P2d.AC13: Concurrency correctness
- **P2d.AC13.1 Success:** Multiple workers claiming the same task — exactly one succeeds (tested)
- **P2d.AC13.2 Success:** Concurrent child node creation under one parent — no ID collisions (tested)
- **P2d.AC13.3 Success:** Simultaneous worker completions — orchestrator handles all correctly (tested)

### P2d.AC14: End-to-end integration
- **P2d.AC14.1 Success:** Full lifecycle test: goal creation → planner breaks into tasks → scheduler assigns work packages → workers execute → tasks complete → session summary generated
- **P2d.AC14.2 Success:** Recovery test: interrupt mid-execution → restart → InProgress tasks reset to Ready → resumes from Scheduling

---

<!-- START_SUBCOMPONENT_A (tasks 1-8) -->

<!-- START_TASK_1 -->
### Task 1: Implement worker spawning (handle_loading, handle_planning, spawn_worker)

**Verifies:** P2d.AC1.1, P2d.AC1.2, P2d.AC1.3, P2d.AC1.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — implement handle_loading, handle_planning, and spawn_worker
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add spawn tests

**Implementation:**

The orchestrator needs access to an LLM client and security infrastructure to create AgentRuntime instances for workers. Add these to the Orchestrator struct:

```rust
use crate::llm::LlmClient;
use crate::security::SecurityValidator;
use crate::security::permission::PermissionHandler;
use crate::tools::factory::create_v2_registry;
use crate::agent::runtime::{AgentRuntime, RuntimeConfig};
use crate::agent::profile::resolve_profile;
use crate::context::ContextBuilder;

pub struct Orchestrator {
    // ... existing fields from Phase 2c ...
    llm_client: Arc<dyn LlmClient>,
    security_validator: Arc<SecurityValidator>,
    permission_handler: Arc<dyn PermissionHandler>,
    project_path: PathBuf,
}
```

Update `Orchestrator::new()` to accept these additional parameters.

**handle_loading:**
1. Query graph store for an active Goal node for this project (status Active)
2. If found: set `self.goal_id`, query children — if Task nodes exist, go to Scheduling, else Planning
3. If not found: create a new Goal node with the user's description, create a new Session, set `self.goal_id` and `self.session_id`, go to Planning

**handle_planning:**
1. Spawn a single planner worker with the "planner" profile
2. The planner's work package contains the goal node as its sole task
3. Wait for the planner to complete (blocking — only one worker in Planning state)
4. After planner completes, check if Task nodes were created under the goal
5. If tasks exist: transition to Scheduling
6. If no tasks: error — planner failed to create tasks

**spawn_worker** — private method:
1. Resolve the agent profile via `resolve_profile()`
2. Create RuntimeConfig from OrchestratorConfig (pass through turn limit, failure thresholds, token budget)
3. Build AgentContext with work package tasks, decisions, handoff notes, AGENTS.md summaries
4. Create a ToolRegistry via `create_v2_registry()`
5. Create AgentRuntime with the LLM client, tools, profile, config
6. Create a CancellationToken
7. Spawn a tokio task that runs `runtime.run(ctx)` and listens for cancellation
8. Create a WorkerHandle and insert into `self.active_workers`
9. Subscribe the worker to the message bus

The spawned task structure:

```rust
let cancel_token = CancellationToken::new();
let cancel_clone = cancel_token.clone();
let handle = tokio::spawn(async move {
    tokio::select! {
        result = runtime.run(ctx) => result,
        _ = cancel_clone.cancelled() => {
            Ok(AgentOutcome::Blocked { reason: "Cancelled by orchestrator".to_string() })
        }
    }
});
```

**Testing:**

Tests use MockLlmClient (from `src/llm/mock.rs`) and Database::open_in_memory():

- P2d.AC1.1: Create orchestrator, call spawn_worker with a work package → verify a WorkerHandle appears in active_workers
- P2d.AC1.3: After spawning, active_workers contains the worker's AgentId
- P2d.AC1.4: WorkerHandle has a CancellationToken that can be triggered

Note: These tests need the orchestrator constructor updated with the additional fields. Create a test helper `create_test_orchestrator()` that uses in-memory DB, MockLlmClient, TokioMessageBus, and a test SecurityValidator.

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): orchestrator worker spawning with AgentRuntime integration`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Implement scheduling and monitoring loops

**Verifies:** P2d.AC2.1, P2d.AC2.2, P2d.AC2.3, P2d.AC2.4, P2d.AC3.1, P2d.AC3.2, P2d.AC3.3, P2d.AC3.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — implement handle_scheduling and handle_monitoring
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**handle_scheduling:**

```rust
async fn handle_scheduling(&mut self) -> Result<OrchestratorState> {
    // 1. Query ready tasks from graph store (status = Ready, under current goal)
    let ready_tasks = self.query_ready_tasks().await?;

    // 2. If no ready tasks and no active workers → check for review or complete
    if ready_tasks.is_empty() && self.active_workers.is_empty() {
        return if self.config.review_required {
            Ok(OrchestratorState::Reviewing)
        } else {
            Ok(OrchestratorState::Completing)
        };
    }

    // 3. If no ready tasks but workers still active → monitor existing workers
    if ready_tasks.is_empty() {
        return Ok(OrchestratorState::Monitoring);
    }

    // 4. Group ready tasks into work packages
    let packages = group_tasks_into_packages(/* ... */);

    // 5. Spawn workers for packages up to capacity
    let available_slots = self.config.max_concurrent_workers
        .saturating_sub(self.active_workers.len());
    for package in packages.into_iter().take(available_slots) {
        // Check file ownership conflicts
        if self.file_locks.acquire(&worker_id, &package.file_scope).is_ok() {
            self.spawn_worker(package).await?;
        }
        // If file conflict, skip this package for now — it'll be picked up next scheduling pass
    }

    Ok(OrchestratorState::Monitoring)
}
```

The `query_ready_tasks` helper:
- Uses `graph_store.query_nodes()` with a NodeQuery filtering for tasks under the goal that have status Ready
- For each task, looks up file_scope from task metadata (stored as comma-separated paths in the node's metadata field)
- Builds a `TaskForGrouping` list

**handle_monitoring:**

The monitoring loop subscribes the orchestrator itself to the message bus and also polls active workers' JoinHandles:

```rust
async fn handle_monitoring(&mut self) -> Result<OrchestratorState> {
    // Subscribe orchestrator to message bus for worker messages
    let mut rx = self.message_bus.subscribe(&"orchestrator".to_string());

    loop {
        tokio::select! {
            // Receive worker messages
            Some(msg) = rx.recv() => {
                match msg {
                    WorkerMessage::TaskCompleted { agent_id, task_id, summary } => {
                        self.handle_task_completed(&agent_id, &task_id, &summary).await?;
                    }
                    WorkerMessage::TaskBlocked { agent_id, task_id, reason } => {
                        self.handle_task_blocked(&agent_id, &task_id, &reason).await?;
                    }
                    WorkerMessage::ProgressReport { agent_id, turn, summary } => {
                        if let Some(handle) = self.active_workers.get_mut(&agent_id) {
                            handle.last_check_in = Utc::now();
                        }
                    }
                    WorkerMessage::NodeCreated { agent_id, parent_id, node } => {
                        // New tasks created by worker — will be picked up in next scheduling pass
                    }
                    WorkerMessage::NeedsDecision { agent_id, task_id, decision } => {
                        // For now: log the decision request. Full handling in later phases.
                    }
                    _ => {} // ReviewRequest/ReviewFeedback handled in Reviewing state
                }
            }
            // Check for completed JoinHandles by polling
            _ = self.poll_worker_completions() => {}
        }

        // Check if we should transition back to Scheduling
        // (a worker finished, freeing slots for more work)
        if self.should_reschedule() {
            return Ok(OrchestratorState::Scheduling);
        }
    }
}
```

`poll_worker_completions` — iterates active_workers, checks if any JoinHandle is finished (using `is_finished()`), processes the result:
- AgentOutcome::Completed → mark tasks completed, release file locks, remove from active_workers
- AgentOutcome::Blocked → mark tasks blocked, release file locks
- AgentOutcome::Failed → error handling (next task)
- AgentOutcome::TokenBudgetExhausted → treat as partial completion
- JoinHandle error (panic/crash) → treat as failure

`should_reschedule` — returns true when any worker finished AND there might be more work to do.

**Testing:**

Tests use MockLlmClient configured to return signal_completion immediately:

- P2d.AC2.1: Spawn a worker that completes → orchestrator marks the task Completed in graph store
- P2d.AC2.4: Spawn a worker, wait for JoinHandle to complete → orchestrator processes the outcome
- P2d.AC3.3: Set max_concurrent_workers=1, try to spawn 2 workers → only 1 spawned
- P2d.AC3.4: No ready tasks, no active workers → orchestrator transitions to Completing

For testing, use MockLlmClient.queue_text_response() or queue_tool_call() to control worker behavior. Create tasks in the in-memory DB before running the orchestrator.

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): orchestrator scheduling and monitoring loops`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Implement error handling, retries, recovery, and token accounting

**Verifies:** P2d.AC4.1, P2d.AC4.2, P2d.AC4.3, P2d.AC4.4, P2d.AC5.1, P2d.AC5.2, P2d.AC6.1, P2d.AC6.2, P2d.AC6.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — add retry logic, recovery, and token tracking
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**Retry logic** — add to the worker completion handler:

```rust
async fn handle_worker_failure(&mut self, agent_id: &AgentId, task_ids: &[String], error: &str) -> Result<()> {
    for task_id in task_ids {
        // Get the task node to check retry count
        let node = self.graph_store.get_node(task_id).await?;
        let retry_count: usize = node.as_ref()
            .and_then(|n| n.metadata.get("retry_count"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        if retry_count < self.config.max_retries_per_task {
            // Reset to Ready with incremented retry count
            // Update node metadata: retry_count = retry_count + 1
            // Update node status to Ready
        } else {
            // Exhausted retries — mark Failed
            // Create an Observation node documenting the failure pattern
        }
    }

    // Release file locks for this worker
    self.file_locks.release(agent_id);
    // Remove worker from active_workers
    self.active_workers.remove(agent_id);
    Ok(())
}
```

Retry metadata is stored on the task node's `metadata` HashMap as `"retry_count" → "N"`. This uses the existing metadata field on GraphNode.

**Recovery logic** — replace the stub from Phase 2c:

```rust
async fn recover_interrupted_session(&mut self, project_id: &str) -> Result<Option<String>> {
    // 1. Query latest session for this project
    let session = self.graph_store.get_latest_session(
        &format!("project:{}", project_id)  // or however sessions reference goals
    ).await?;

    if let Some(session) = session {
        if session.completed_at.is_none() {
            // Interrupted session found
            self.goal_id = Some(session.goal_id.clone());
            self.session_id = Some(session.id.clone());

            // Reset InProgress tasks to Ready
            let in_progress_tasks = self.graph_store.query_nodes(/* NodeQuery for InProgress tasks */).await?;
            for task in in_progress_tasks {
                // Update status to Ready
                self.graph_store.update_node(&task.id, /* status = Ready */).await?;
            }

            return Ok(Some(session.goal_id));
        }
    }
    Ok(None)
}
```

Update handle_startup to call recover_interrupted_session:

```rust
async fn handle_startup(&mut self, project_id: &str) -> Result<OrchestratorState> {
    if let Some(goal_id) = self.recover_interrupted_session(project_id).await? {
        self.goal_id = Some(goal_id);
        return Ok(OrchestratorState::Scheduling); // Resume directly
    }
    Ok(OrchestratorState::Loading)
}
```

**Token accounting:**

Add a `tokens_used: usize` field to `AgentOutcome::Completed` in `src/agent/mod.rs`:

```rust
pub enum AgentOutcome {
    Completed { summary: String, tokens_used: usize },
    // ... other variants unchanged
}
```

Update `AgentRuntime::run()` in `src/agent/runtime.rs` to populate `tokens_used` from the cumulative token count tracked during the agentic loop.

The orchestrator extracts `tokens_used` from each completed worker's `AgentOutcome::Completed` and adds it to `self.cumulative_tokens`. This is the canonical source — no need to query worker_conversations separately for token counts.

Before spawning new workers in handle_scheduling, check:

```rust
if let Some(max) = self.config.max_tokens_per_goal {
    if self.cumulative_tokens >= max {
        return Ok(OrchestratorState::Completing); // Budget exceeded
    }
}
```

**Testing:**

- P2d.AC4.1: Create orchestrator with max_retries=2, spawn worker that fails → task gets retry_count=1, status reset to Ready
- P2d.AC4.2: After first failure, the task is Ready and can be rescheduled
- P2d.AC4.3: Set max_retries=0, worker fails → task marked Failed, Observation node created
- P2d.AC5.1: Create a session with InProgress tasks, call recover_interrupted_session → tasks reset to Ready
- P2d.AC5.2: After recovery, orchestrator state is Scheduling
- P2d.AC6.1: Spawn two workers that complete with token counts → cumulative_tokens reflects sum

These tests require setting up graph nodes in the in-memory DB first, then running specific orchestrator methods.

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): orchestrator error handling, retries, recovery, and token accounting`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Enhance built-in profiles with structured system prompts

**Verifies:** P2d.AC7.1, P2d.AC7.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/builtin_profiles.rs` — update system prompts to structured template
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add profile prompt tests

**Implementation:**

Update all 5 built-in profiles in `src/agent/builtin_profiles.rs` to use the structured system prompt template from the architecture (lines 1549-1575). The template structure is:

```
You are a {role} agent working on project "{project_name}".

## Your Task
{task_description}

## Acceptance Criteria
{acceptance_criteria}

## Context
{relevant_decisions}
{agents_md_content}

## Rules
- {profile-specific rules}
- When you make a non-trivial choice between alternatives, log a decision using log_decision.
- When you discover something noteworthy, record it using record_observation.
- Signal completion or blocking using the signal tool. Do not simply stop.
```

The `system_prompt` field becomes a template string with `{{project_name}}`, `{{task_description}}`, `{{acceptance_criteria}}`, `{{relevant_decisions}}`, `{{agents_md_content}}` placeholders. The ContextBuilder (called in spawn_worker) renders these placeholders at runtime.

Profile-specific rules per the architecture:
- **planner**: "Break work into tasks that can be completed independently. Keep tasks small enough for a single focused session. Specify acceptance criteria for every task."
- **coder**: "Check your work against the acceptance criteria before signaling completion. Only modify files within your declared scope. Commit logical units of work."
- **reviewer**: "Do not modify files. Report issues as Observation nodes. Approve or reject via the signal tool with specific feedback."
- **tester**: "Write tests that verify behavior, not implementation details. Test edge cases and error conditions. Ensure tests are clear and maintainable."
- **researcher**: "Document all findings as Observation nodes. Provide specific file paths and line numbers. Organize findings by relevance to the goal."

Also add "agent" to the `allowed_tools` for coder and tester profiles (for multi-agent spawn_sub_agent/send_message/query_agent_status tools from Phase 2e).

**Testing:**

- P2d.AC7.1: Each built-in profile's system_prompt contains "## Your Task", "## Acceptance Criteria", "## Context", "## Rules" sections
- P2d.AC7.2: Planner's rules contain "independently"; coder's contain "declared scope"; reviewer's contain "Do not modify"

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): enhance built-in profiles with structured system prompt template`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Add check-in interval support to AgentRuntime and file scope enforcement

**Verifies:** P2d.AC8.1, P2d.AC8.2, P2d.AC10.1, P2d.AC10.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/runtime.rs` — add MessageBus to RuntimeConfig, send ProgressReport every N turns
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/security/mod.rs` — add FileOwnershipMap-aware validation
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**Check-in intervals:**

Add optional message bus fields to `RuntimeConfig`:

```rust
pub struct RuntimeConfig {
    // ... existing fields ...
    pub message_bus: Option<Arc<dyn MessageBus>>,
    pub agent_id: Option<AgentId>,
    pub check_in_interval: usize, // default: 10
}
```

In the agentic loop (`AgentRuntime::run()`), after each turn, check if `turn_count % check_in_interval == 0`. If so, and if `message_bus` is Some, send a `WorkerMessage::ProgressReport`:

```rust
if let (Some(bus), Some(id)) = (&self.config.message_bus, &self.config.agent_id) {
    if turn_count % self.config.check_in_interval == 0 {
        let _ = bus.send(&"orchestrator".to_string(), WorkerMessage::ProgressReport {
            agent_id: id.clone(),
            turn: turn_count,
            summary: format!("Turn {}: processing", turn_count),
        }).await;
    }
}
```

The `send` result is ignored (fire-and-forget semantics).

**File scope enforcement:**

Add an optional `FileOwnershipMap` reference and `AgentId` to `SecurityValidator` or create a wrapper that combines both. The simplest approach: add an `ownership_check` closure to the file tools.

In `src/tools/file.rs`, the `WriteFileTool::execute()` should check file ownership before writing:

```rust
// In the write_file tool's execute method, before performing the write:
if let Some(ownership) = &self.file_ownership {
    if !ownership.can_write(&self.agent_id, &path) {
        return Ok(format!("Error: file {} is outside your declared scope or owned by another worker. Signal NeedsDecision to request scope expansion.", path.display()));
    }
}
```

Add optional `file_ownership: Option<Arc<Mutex<FileOwnershipMap>>>` and `agent_id: Option<AgentId>` fields to `WriteFileTool`. These are set when creating the tool registry for multi-agent workers (via `create_v2_registry`) and left as None for single-agent mode.

**Testing:**

- P2d.AC8.1: Create AgentRuntime with message_bus + check_in_interval=2, run for 4 turns → at least 2 ProgressReport messages received
- P2d.AC10.1: Create WriteFileTool with FileOwnershipMap, agent owns "src/main.rs" → write to "src/main.rs" succeeds
- P2d.AC10.2: Same setup, write to "src/other.rs" (owned by different agent) → returns error string about scope

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): check-in intervals and file scope enforcement for workers`

<!-- END_TASK_5 -->

<!-- START_TASK_6 -->
### Task 6: Implement reviewing state and worker conversation persistence

**Verifies:** P2d.AC9.1, P2d.AC9.2, P2d.AC11.1, P2d.AC11.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — implement handle_reviewing, add conversation persistence
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/runtime.rs` — return conversation history in AgentOutcome
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**handle_reviewing:**

Replace the `todo!()` stub with a real implementation:

```rust
async fn handle_reviewing(&mut self) -> Result<OrchestratorState> {
    if !self.config.review_required {
        return Ok(OrchestratorState::Completing);
    }

    // Find completed work packages that haven't been reviewed yet
    // (check metadata for "reviewed" flag on completed task nodes)
    let unreviewed = self.find_unreviewed_completed_tasks().await?;

    if unreviewed.is_empty() {
        return Ok(OrchestratorState::Completing);
    }

    // Spawn reviewer worker(s) for unreviewed work
    for task_group in unreviewed {
        let review_package = WorkPackage {
            id: generate_work_package_id(),
            task_ids: task_group.iter().map(|t| t.id.clone()).collect(),
            file_scope: /* gather file scopes from completed tasks */,
            profile: "reviewer".to_string(),
            priority: Priority::High,
            estimated_complexity: Complexity::Medium,
        };
        self.spawn_worker(review_package).await?;
    }

    // After spawning reviewers, go to Monitoring to wait for them
    Ok(OrchestratorState::Monitoring)
}
```

After reviewers complete, the orchestrator returns to Scheduling (to handle any new tasks created from review feedback), then eventually back to Reviewing or Completing.

**Worker conversation persistence:**

On worker completion (in `poll_worker_completions`), write the conversation to the `worker_conversations` table:

```rust
// After extracting AgentOutcome from JoinHandle:
self.graph_store.execute(|conn| {
    conn.execute(
        "INSERT INTO worker_conversations (id, session_id, agent_id, task_ids, messages, total_input_tokens, total_output_tokens, started_at, completed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, session_id, agent_id, task_ids_json, messages_json, input_tokens, output_tokens, started_at, completed_at],
    )
}).await?;
```

The conversation messages are collected by AgentRuntime during the agentic loop and returned alongside the AgentOutcome. Add a `conversation: Vec<Message>` field to AgentOutcome variants, or return it as a separate value from `runtime.run()` (returning `(AgentOutcome, Vec<Message>)`).

**Testing:**

- P2d.AC11.1: Set review_required=true, complete a worker → handle_reviewing spawns a reviewer
- P2d.AC11.2: After reviewer completes, orchestrator transitions to Scheduling or Completing
- P2d.AC9.1: Worker completes → conversation written to worker_conversations table (query to verify)
- P2d.AC9.2: Verify the record has correct session_id, agent_id, and token counts

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): reviewing state handler and worker conversation persistence`

<!-- END_TASK_6 -->

<!-- START_TASK_7 -->
### Task 7: Implement dynamic adaptation (file scope expansion, task splitting)

**Verifies:** P2d.AC12.1, P2d.AC12.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — handle NeedsDecision for scope expansion, discover worker-created subtasks
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add tests

**Implementation:**

**File scope expansion** (in handle_monitoring's NeedsDecision handler):

```rust
WorkerMessage::NeedsDecision { agent_id, task_id, decision } => {
    // Check if this is a file scope expansion request
    // (decision node's metadata contains "requested_files")
    if let Some(requested_files) = decision.metadata.get("requested_files") {
        let files: Vec<PathBuf> = /* parse comma-separated paths */;
        // Check if any requested file conflicts with another active worker
        let can_expand = files.iter().all(|f| self.file_locks.can_write(&agent_id, f));
        if can_expand {
            // Expand scope: acquire new files for this agent
            self.file_locks.acquire(&agent_id, &files)?;
            // Notify worker via AdditionalContext
            self.message_bus.send(&agent_id, WorkerMessage::AdditionalContext {
                content: format!("Scope expanded: you now have access to {}", requested_files),
            }).await?;
        } else {
            // Conflict — re-queue the task for after conflicting worker finishes
            // Mark task as Ready, cancel the current worker
            self.handle_worker_failure(&agent_id, &[task_id.clone()], "file scope conflict").await?;
        }
    }
}
```

**Task splitting discovery** (in handle_scheduling):

The scheduling loop already queries ready tasks from the graph store. Worker-created subtask nodes (via `create_node` tool) have status Ready and appear naturally in the next `query_ready_tasks()` call. No additional code needed — but add a test to verify this behavior explicitly.

**Testing:**

- P2d.AC12.1: Worker sends NeedsDecision with requested_files, no conflict → scope expanded, worker receives AdditionalContext
- P2d.AC12.1 (conflict): Worker sends NeedsDecision, files owned by another worker → task re-queued
- P2d.AC12.2: Worker creates subtask nodes during execution → subtasks appear in next scheduling pass as Ready tasks

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): dynamic adaptation with file scope expansion and task splitting`

<!-- END_TASK_7 -->

<!-- START_TASK_8 -->
### Task 8: Concurrency tests and end-to-end integration test

**Verifies:** P2d.AC13.1, P2d.AC13.2, P2d.AC13.3, P2d.AC14.1, P2d.AC14.2

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_e2e_test.rs` — end-to-end and concurrency tests
- Test: uses Database::open_in_memory(), MockLlmClient, TokioMessageBus

**Implementation:**

**Concurrency tests:**

These tests verify race conditions that matter (per the architecture's Testing Strategy, line 2120):

- P2d.AC13.1: Multiple workers claiming the same task — use `claim_task` graph tool from multiple concurrent tasks. Exactly one should succeed (the graph store's `BEGIN IMMEDIATE` transaction serializes claims).

```rust
#[tokio::test]
async fn test_concurrent_task_claiming() {
    // Create a Ready task
    // Spawn N tasks that each try to claim_task
    // Assert exactly 1 succeeds and N-1 get conflict errors
}
```

- P2d.AC13.2: Concurrent child creation — spawn multiple tokio tasks that call `generate_child_id` and `create_node` for the same parent. Verify no duplicate IDs.

- P2d.AC13.3: Simultaneous completions — create orchestrator with 2 workers, both complete at roughly the same time (use MockLlmClient with instant completion). Verify both outcomes are processed correctly.

**End-to-end integration test:**

Uses MockLlmClient configured to simulate a planner that creates tasks, and workers that complete them:

```rust
#[tokio::test]
async fn test_full_orchestrator_lifecycle() {
    // Setup: in-memory DB, mock LLM that:
    //   - As planner: calls create_node to make 2 tasks, then signal_completion
    //   - As coder: calls signal_completion immediately
    // Create orchestrator with max_concurrent_workers=2
    // Run orchestrator with a goal description
    // Verify:
    //   1. Goal node created
    //   2. Planner spawned (Planning state)
    //   3. Tasks created by planner
    //   4. Workers spawned for tasks (Scheduling → Monitoring)
    //   5. Tasks marked Completed
    //   6. Session summary generated (Completing state)
}
```

**Recovery test:**

```rust
#[tokio::test]
async fn test_recovery_after_interruption() {
    // Setup: in-memory DB with a goal, session, and InProgress tasks
    // Create orchestrator, run it
    // Verify:
    //   1. Startup detects interrupted session
    //   2. InProgress tasks reset to Ready
    //   3. Orchestrator resumes from Scheduling (not Planning)
    //   4. Tasks eventually complete
}
```

**Verification:**

Run: `cargo test orchestrator_e2e_test`
Expected: All tests pass

**Commit:** `test(agent): concurrency tests and end-to-end orchestrator integration test`

<!-- END_TASK_8 -->
<!-- END_SUBCOMPONENT_A -->
