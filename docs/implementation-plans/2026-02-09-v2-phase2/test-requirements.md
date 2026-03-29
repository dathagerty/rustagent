# Test Requirements for V2 Phase 2

This document maps every acceptance criterion from Phase 2a through Phase 2g to specific automated tests or documented human verification steps. Each criterion is traced to the implementation plan task that produces it and the test file where verification lives.

The V2 Phase 2 architecture covers Multi-Agent Orchestration: the message bus, work packages, orchestrator state machine, runtime integration, agent tools, git worktree isolation, and CLI updates.

---

## Phase 2a: Message Bus

### P2a.AC1: WorkerMessage types

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2a.AC1.1 | integration | `tests/message_test.rs` | Construct each of the 9 WorkerMessage variants (ProgressReport, TaskCompleted, TaskBlocked, NeedsDecision, NodeCreated, Cancel, AdditionalContext, ReviewRequest, ReviewFeedback) -- verifies the enum compiles with correct field names. |
| P2a.AC1.2 | integration | `tests/message_test.rs` | Each variant carries the correct fields as specified in the architecture. Verified implicitly by AC1.1's construction tests, which use named fields matching the spec (e.g., `ProgressReport { agent_id, turn, summary }`). |
| P2a.AC1.3 | integration | `tests/message_test.rs` | Clone a WorkerMessage variant, format it with `Debug`. Assert both operations succeed at runtime, verifying Clone + Debug. Send + Sync are compile-time properties verified by trait bound on MessageBus. |

### P2a.AC2: MessageBus trait

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2a.AC2.1 | integration | `tests/message_test.rs` | Verified structurally: TokioMessageBus implements the MessageBus trait with `send()`, `broadcast()`, and `subscribe()` methods. If any method is missing, compilation fails. All subsequent tests exercise these methods. |
| P2a.AC2.2 | integration | `tests/message_test.rs` | Create TokioMessageBus, subscribe agent "a1", send a targeted message to "a1", receive it on the subscriber's receiver. Assert the received message matches. |
| P2a.AC2.3 | integration | `tests/message_test.rs` | Subscribe two agents, broadcast a message. Both receivers get the message. |
| P2a.AC2.4 | integration | `tests/message_test.rs` | `subscribe()` returns a receiver that gets both targeted messages (via `send`) and broadcast messages. Verified by sending a targeted message and a broadcast to the same subscriber, confirming both arrive on the single receiver. |

### P2a.AC3: TokioMessageBus implementation

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2a.AC3.1 | integration | `tests/message_test.rs` | Structural verification: TokioMessageBus uses `tokio::sync::broadcast` for fan-out and per-agent `tokio::sync::mpsc` for targeted delivery. Verified by the behavior tests (AC2.2, AC2.3, AC3.3) which demonstrate that broadcast reaches all subscribers while targeted delivery reaches only one. |
| P2a.AC3.2 | integration | `tests/message_test.rs` | Subscribe 3 agents, broadcast a message. All 3 receive it. |
| P2a.AC3.3 | integration | `tests/message_test.rs` | Subscribe two agents "a1" and "a2". Send targeted message to "a1". Verify "a1" receives it and "a2" does not (use `tokio::time::timeout` to confirm "a2" gets nothing within 100ms). |
| P2a.AC3.4 | integration | `tests/message_test.rs` | Broadcast a message before any subscription. Then subscribe an agent. Use `tokio::time::timeout` to confirm the subscriber does not receive the earlier message (fire-and-forget semantics). |
| P2a.AC3.5 | integration | `tests/message_test.rs` | Subscribe an agent, drop the receiver. Broadcast a message. Bus does not panic. Also test `remove_subscriber`: subscribe "a1", remove it, send to "a1" -- returns error (agent not found). |

**Implementation task:** Phase 2a, Task 2 (Create message module with WorkerMessage and TokioMessageBus).

---

## Phase 2b: WorkPackage + File Ownership

### P2b.AC1: WorkPackage type

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2b.AC1.1 | unit | `tests/work_package_test.rs` | Construct a WorkPackage with all fields (id, task_ids, file_scope, profile, priority, estimated_complexity). Assert fields are accessible and have correct values. |
| P2b.AC1.2 | unit | `tests/work_package_test.rs` | Construct each Complexity variant (Small, Medium, Large). Assert they are distinct values and implement Debug. |
| P2b.AC1.3 | unit | `tests/work_package_test.rs` | WorkPackage uses the existing `graph::Priority` enum. Construct a WorkPackage with `Priority::High` and verify the field type. |

### P2b.AC2: FileOwnershipMap

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2b.AC2.1 | unit | `tests/work_package_test.rs` | Create FileOwnershipMap, acquire files for agent "a1". Verify `can_write("a1", file)` returns true. |
| P2b.AC2.2 | unit | `tests/work_package_test.rs` | Acquire files for "a1", attempt to acquire the same files for "a2". Returns Err with a message indicating the file is already owned. |
| P2b.AC2.3 | unit | `tests/work_package_test.rs` | Acquire files for "a1", release "a1", then acquire the same files for "a2". Succeeds without error. |
| P2b.AC2.4 | unit | `tests/work_package_test.rs` | Acquire "src/main.rs" for "a1". `can_write("a1", "src/main.rs")` returns true. |
| P2b.AC2.5 | unit | `tests/work_package_test.rs` | Acquire "src/main.rs" for "a1". `can_write("a2", "src/main.rs")` returns false. |
| P2b.AC2.6 | unit | `tests/work_package_test.rs` | Empty map. `can_write("a1", "src/anything.rs")` returns true (unowned files are writable). |

### P2b.AC3: WorkerHandle and WorkerState

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2b.AC3.1 | integration | `tests/work_package_test.rs` | Construct a WorkerHandle with all fields (id, profile, work_package, state, join_handle, cancel_token, spawned_at, last_check_in) using a dummy `tokio::spawn` JoinHandle. Assert id and profile are accessible. Uses `#[tokio::test]`. |
| P2b.AC3.2 | unit | `tests/work_package_test.rs` | Construct each WorkerState variant (Spawning, Initializing, Working, Reporting, Completed, Failed). Assert Debug formatting works for all variants. |

### P2b.AC4: Task grouping logic

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2b.AC4.1 | unit | `tests/work_package_test.rs` | Two tasks sharing "src/main.rs" in their file_scope. `group_tasks_into_packages()` returns 1 work package containing both tasks. |
| P2b.AC4.2 | unit | `tests/work_package_test.rs` | Task A depends on Task B (both in ready set). `group_tasks_into_packages()` returns 1 work package containing both. |
| P2b.AC4.3 | unit | `tests/work_package_test.rs` | Two tasks with completely separate file scopes and no dependencies. `group_tasks_into_packages()` returns 2 work packages. Also verify generated work package IDs match `^wp-[0-9a-f]{8}$`. |

**Implementation tasks:** Phase 2b, Task 1 (WorkPackage, Complexity, FileOwnershipMap), Task 2 (WorkerHandle and WorkerState), Task 3 (Task grouping logic).

---

## Phase 2c: Orchestrator Core State Machine

### P2c.AC1: OrchestratorConfig

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2c.AC1.1 | unit | `tests/orchestrator_test.rs` | Construct OrchestratorConfig with all fields (max_concurrent_workers, max_retries_per_task, worker_turn_limit, check_in_interval, review_required, max_consecutive_llm_failures, max_consecutive_tool_failures, worker_token_budget, token_budget_warning_pct, max_tokens_per_goal). Assert all fields are accessible. |
| P2c.AC1.2 | unit | `tests/orchestrator_test.rs` | `OrchestratorConfig::default()` returns documented defaults: max_concurrent_workers=4, max_retries_per_task=2, worker_turn_limit=100, check_in_interval=10, review_required=false, max_consecutive_llm_failures=3, max_consecutive_tool_failures=3, worker_token_budget=200_000, token_budget_warning_pct=80, max_tokens_per_goal=None. Assert each field individually. |

### P2c.AC2: OrchestratorState

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2c.AC2.1 | unit | `tests/orchestrator_test.rs` | Construct each of the 7 OrchestratorState variants (Startup, Loading, Planning, Scheduling, Monitoring, Reviewing, Completing). Format with Debug to verify they exist. |
| P2c.AC2.2 | unit | `tests/orchestrator_test.rs` | Verify state transitions follow the defined state machine graph. Create orchestrator, check initial state is Startup. Manually set state to each valid transition to confirm the enum works. Full run-loop transition testing deferred to Phase 2d integration tests. |

### P2c.AC3: Orchestrator struct

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2c.AC3.1 | integration | `tests/orchestrator_test.rs` | Create Orchestrator with mock GraphStore and mock MessageBus. Verify all fields are accessible (config, state, graph_store, message_bus, active_workers, file_locks). |
| P2c.AC3.2 | integration | `tests/orchestrator_test.rs` | `Orchestrator::new()` initializes in Startup state. Assert `orchestrator.state() == &OrchestratorState::Startup`. |

### P2c.AC4: Recovery logic

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2c.AC4.1 | deferred | `tests/orchestrator_test.rs` | Deferred to Phase 2d (P2d.AC5.1). This phase provides the method stub; Phase 2d fills in the implementation and tests. |
| P2c.AC4.2 | deferred | `tests/orchestrator_test.rs` | Deferred to Phase 2d (P2d.AC5.2). This phase provides the method stub; Phase 2d fills in the implementation and tests. |

**Implementation tasks:** Phase 2c, Task 1 (OrchestratorConfig and OrchestratorState), Task 2 (Orchestrator struct with state machine skeleton).

---

## Phase 2d: Orchestrator-Runtime Integration

### P2d.AC1: Worker spawning

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC1.1 | integration | `tests/orchestrator_test.rs` | Create orchestrator, call `spawn_worker` with a work package. Verify a WorkerHandle appears in `active_workers`. Uses MockLlmClient and Database::open_in_memory(). |
| P2d.AC1.2 | integration | `tests/orchestrator_test.rs` | Verify each spawned worker's AgentContext has correct `work_package_tasks`, `project_path`, `profile`, and `graph_store`. Inspected through the test by verifying the worker runs and interacts with the correct graph store. |
| P2d.AC1.3 | integration | `tests/orchestrator_test.rs` | After spawning, `active_workers` map contains the worker's AgentId as a key. |
| P2d.AC1.4 | integration | `tests/orchestrator_test.rs` | WorkerHandle has a CancellationToken that can be triggered. Cancel a worker's token and verify the JoinHandle resolves with a Blocked outcome. |

### P2d.AC2: Message handling in Monitoring state

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC2.1 | integration | `tests/orchestrator_test.rs` | Spawn a worker that sends TaskCompleted. Orchestrator processes it and marks the task Completed in the graph store. Verify via `graph_store.get_node()`. |
| P2d.AC2.2 | integration | `tests/orchestrator_test.rs` | Spawn a worker that sends TaskBlocked. Orchestrator processes it and marks the task Blocked in the graph store. |
| P2d.AC2.3 | integration | `tests/orchestrator_test.rs` | Spawn a worker configured to send ProgressReport messages. Verify the orchestrator updates `last_check_in` on the WorkerHandle. |
| P2d.AC2.4 | integration | `tests/orchestrator_test.rs` | Spawn a worker, wait for its JoinHandle to complete. Verify the orchestrator processes the AgentOutcome (removes worker from active_workers, releases file locks). |

### P2d.AC3: Scheduling logic

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC3.1 | integration | `tests/orchestrator_test.rs` | Create Ready tasks in the graph store under the goal. Run scheduling. Verify the orchestrator queries and discovers them for work package creation. |
| P2d.AC3.2 | integration | `tests/orchestrator_test.rs` | Create multiple Ready tasks with overlapping file scopes. Scheduling groups them into work packages using `group_tasks_into_packages`. |
| P2d.AC3.3 | integration | `tests/orchestrator_test.rs` | Set `max_concurrent_workers=1`. Create 2 Ready tasks. Scheduling spawns only 1 worker. The second is deferred to the next scheduling pass. |
| P2d.AC3.4 | integration | `tests/orchestrator_test.rs` | No ready tasks remain and no workers are active. Orchestrator transitions to Completing (or Reviewing if review_required). |

### P2d.AC4: Error handling and retries

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC4.1 | integration | `tests/orchestrator_test.rs` | Create orchestrator with `max_retries_per_task=2`. Spawn a worker that fails. Verify the task node's metadata has `retry_count=1` and its status is reset to Ready. |
| P2d.AC4.2 | integration | `tests/orchestrator_test.rs` | After first failure, the task is Ready and eligible for re-scheduling in the next scheduling pass. |
| P2d.AC4.3 | integration | `tests/orchestrator_test.rs` | Set `max_retries_per_task=0`. Worker fails. Task is marked Failed and an Observation node documenting the failure is created as a child. |
| P2d.AC4.4 | integration | `tests/orchestrator_test.rs` | Spawn a worker whose JoinHandle returns an error (simulated panic). Orchestrator treats it identically to a persistent failure (retries or marks Failed). |

### P2d.AC5: Recovery logic

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC5.1 | integration | `tests/orchestrator_test.rs` | Create a session with InProgress tasks in the graph store (simulating interrupted execution). Create orchestrator and call `recover_interrupted_session`. Verify InProgress tasks are reset to Ready. |
| P2d.AC5.2 | integration | `tests/orchestrator_test.rs` | After recovery, orchestrator state transitions to Scheduling (skipping Loading/Planning). |

### P2d.AC6: Token accounting

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC6.1 | integration | `tests/orchestrator_test.rs` | Spawn two workers that complete with token counts (via MockLlmClient with configured token usage). Verify `cumulative_tokens` reflects the sum of both workers' tokens. |
| P2d.AC6.2 | integration | `tests/orchestrator_test.rs` | Set `max_tokens_per_goal = Some(500)`. First worker uses 400 tokens. Second worker uses 200 tokens (total 600 > 500). Orchestrator detects budget exceeded and transitions to Completing. |
| P2d.AC6.3 | unit | `tests/agent_types_test.rs` | `AgentOutcome::Completed` variant includes `tokens_used: usize` field. Construct and pattern-match to verify. AgentRuntime populates this field from cumulative token tracking. |

### P2d.AC7: Built-in profile enhancement

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC7.1 | unit | `tests/orchestrator_test.rs` | Each built-in profile's `system_prompt` template contains the structured sections: "## Your Task", "## Acceptance Criteria", "## Context", "## Rules". Verify by string containment on all 5 profiles. |
| P2d.AC7.2 | unit | `tests/orchestrator_test.rs` | Profile-specific rules: planner's rules contain "independently"; coder's contain "declared scope"; reviewer's contain "Do not modify"; tester's contain "behavior"; researcher's contain "findings". Verify by string containment. |

### P2d.AC8: Check-in interval support

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC8.1 | integration | `tests/orchestrator_test.rs` | Create AgentRuntime with `message_bus` and `check_in_interval=2`. Run for 4+ turns using MockLlmClient. Verify at least 2 ProgressReport messages were sent to the message bus. |
| P2d.AC8.2 | unit | `tests/orchestrator_test.rs` | RuntimeConfig includes `message_bus: Option<Arc<dyn MessageBus>>` and `agent_id: Option<AgentId>`. Construct with None for both (single-agent mode) -- compiles and runs without errors. Construct with Some values for both (multi-agent mode). |

### P2d.AC9: Worker conversation persistence

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC9.1 | integration | `tests/orchestrator_test.rs` | Worker completes. Query `worker_conversations` table. Verify a record exists with the worker's conversation (messages JSON). |
| P2d.AC9.2 | integration | `tests/orchestrator_test.rs` | Verify the `worker_conversations` record has correct `session_id`, `agent_id`, `task_ids` (JSON array), `total_input_tokens`, and `total_output_tokens` matching the worker's actual usage. |

### P2d.AC10: File scope enforcement

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC10.1 | integration | `tests/orchestrator_test.rs` | Create WriteFileTool with FileOwnershipMap. Agent "a1" owns "src/main.rs". Write to "src/main.rs" via the tool succeeds. |
| P2d.AC10.2 | integration | `tests/orchestrator_test.rs` | Same setup. Agent "a1" writes to "src/other.rs" (owned by agent "a2"). Tool returns an error string about the file being outside declared scope. |

### P2d.AC11: Reviewing state

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC11.1 | integration | `tests/orchestrator_test.rs` | Set `review_required=true`. Complete a coder worker. `handle_reviewing` spawns a reviewer worker for the completed work package. |
| P2d.AC11.2 | integration | `tests/orchestrator_test.rs` | After reviewer completes, orchestrator transitions to Scheduling (if new tasks from review) or Completing. |

### P2d.AC12: Dynamic adaptation

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC12.1 | integration | `tests/orchestrator_test.rs` | Worker sends NeedsDecision with `requested_files` in metadata. No file conflict exists. Orchestrator expands scope via `file_locks.acquire()` and sends AdditionalContext to the worker. Also test the conflict case: files owned by another worker results in task re-queueing. |
| P2d.AC12.2 | integration | `tests/orchestrator_test.rs` | Worker creates subtask nodes during execution via `create_node` tool. In the next scheduling pass, the orchestrator discovers the new Ready tasks and considers them for work package creation. |

### P2d.AC13: Concurrency correctness

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC13.1 | integration | `tests/orchestrator_e2e_test.rs` | Spawn 10 concurrent tokio tasks that each call `claim_task` for the same Ready task. Assert exactly 1 succeeds and 9 fail. SQLite's `BEGIN IMMEDIATE` serializes the claims. |
| P2d.AC13.2 | integration | `tests/orchestrator_e2e_test.rs` | Spawn multiple tokio tasks that each call `generate_child_id` and `create_node` for the same parent concurrently. Verify no duplicate IDs are generated. |
| P2d.AC13.3 | integration | `tests/orchestrator_e2e_test.rs` | Create orchestrator with 2 workers (MockLlmClient with instant completion). Both complete simultaneously. Verify both AgentOutcomes are processed correctly, both tasks marked Completed, file locks released for both. |

### P2d.AC14: End-to-end integration

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2d.AC14.1 | integration | `tests/orchestrator_e2e_test.rs` | Full lifecycle: goal creation -> planner spawned (creates 2 tasks via create_node tool) -> scheduler assigns work packages -> workers execute (MockLlmClient calls signal_completion) -> tasks marked Completed -> session summary generated. Verify each step via graph store queries. |
| P2d.AC14.2 | integration | `tests/orchestrator_e2e_test.rs` | Recovery: set up in-memory DB with a goal, session, and InProgress tasks. Create orchestrator, run it. Verify: startup detects interrupted session, InProgress tasks reset to Ready, resumes from Scheduling (not Planning), tasks eventually complete. |

**Implementation tasks:** Phase 2d, Tasks 1-8.

---

## Phase 2e: Agent Tools

### P2e.AC1: spawn_sub_agent tool

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2e.AC1.1 | integration | `tests/agent_tools_test.rs` | Call `SpawnSubAgentTool::execute()` with valid params (title, description, parent_task_id). Verify a new task node exists in the graph store as a child of the parent. |
| P2e.AC1.2 | integration | `tests/agent_tools_test.rs` | After execute, verify a NodeCreated message was broadcast on the message bus. Use a subscriber to confirm receipt. |
| P2e.AC1.3 | integration | `tests/agent_tools_test.rs` | Return value from execute is JSON containing the new `task_id`. Parse and verify it matches the node created in the graph store. |
| P2e.AC1.4 | integration | `tests/agent_tools_test.rs` | New task node has status Ready. Verify via `graph_store.get_node(task_id)`. |

### P2e.AC2: send_message tool

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2e.AC2.1 | integration | `tests/agent_tools_test.rs` | Create SendMessageTool, subscribe a target agent on the message bus. Send a message via the tool. Target agent receives the message on its subscriber. |
| P2e.AC2.2 | integration | `tests/agent_tools_test.rs` | Send a ReviewRequest message type (with work_package_id and changed_files). Verify the received message is a `WorkerMessage::ReviewRequest`. Also test ReviewFeedback message type. |
| P2e.AC2.3 | integration | `tests/agent_tools_test.rs` | Send to a non-existent agent ID (no subscriber). Tool returns a descriptive error string (not a panic or Err). |

### P2e.AC3: query_agent_status tool

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2e.AC3.1 | integration | `tests/agent_tools_test.rs` | Create task nodes in the graph store with `assigned_to = "worker-1"`. Call `QueryAgentStatusTool::execute()` with agent_id "worker-1". Returned JSON lists the tasks with correct statuses and titles. |
| P2e.AC3.2 | integration | `tests/agent_tools_test.rs` | Query for non-existent agent "worker-999". Returns JSON with an empty tasks array and a "no tasks found" note. |

### P2e.AC4: Tool registration

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2e.AC4.1 | integration | `tests/agent_tools_test.rs` | Create v2 registry with `message_bus = Some(...)` and `agent_id = Some(...)`. Registry contains tools named "spawn_sub_agent", "send_message", "query_agent_status". |
| P2e.AC4.2 | integration | `tests/agent_tools_test.rs` | Create v2 registry with `message_bus = None` and `agent_id = None` (single-agent mode backward compat). Registry does NOT contain agent tools. Verify the existing tools (file, shell, signal, graph) are still present. |

**Implementation tasks:** Phase 2e, Tasks 1-3.

---

## Phase 2f: Git Worktree Integration

### P2f.AC1: Goal branch management

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2f.AC1.1 | integration | `tests/worktree_test.rs` | Create a temp dir with `git init`. Call `WorktreeManager::create_goal_branch("ra-a1b2")`. Verify branch `rustagent/ra-a1b2` exists in `git branch --list`. Uses `tempfile::TempDir`. |
| P2f.AC1.2 | integration | `tests/worktree_test.rs` | Call `create_goal_branch` twice with the same goal ID. Second call succeeds without error (branch already exists, reused). |
| P2f.AC1.3 | human | N/A | See Human Verification table below. |

### P2f.AC2: Worktree creation for work packages

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2f.AC2.1 | integration | `tests/worktree_test.rs` | Create goal branch, then call `create_worktree("ra-a1b2", "abc12345")`. Verify the worktree directory exists at `.rustagent/worktrees/ra-a1b2-wp-abc12345/`. |
| P2f.AC2.2 | integration | `tests/worktree_test.rs` | After `create_worktree`, verify `git branch --list` shows branch `rustagent/ra-a1b2/wp-abc12345`. |
| P2f.AC2.3 | integration | `tests/orchestrator_test.rs` | Spawn a worker with worktree manager configured. Verify the worker's `AgentContext.project_path` is the worktree path (not the main project path). |

### P2f.AC3: Worktree merge and cleanup

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2f.AC3.1 | integration | `tests/worktree_test.rs` | Create goal branch, create worktree, write a file in the worktree, commit it. Call `merge_work_package`. Verify the committed file is present on the goal branch. |
| P2f.AC3.2 | integration | `tests/worktree_test.rs` | After successful merge, call `cleanup_worktree`. Verify the worktree directory no longer exists and the wp branch is deleted. |
| P2f.AC3.3 | integration | `tests/worktree_test.rs` | Create a merge conflict scenario: modify the same file on both the goal branch and the worktree branch. Call `merge_work_package`. Verify it returns an error containing "conflict". Verify the worktree is preserved. |

### P2f.AC4: Single-agent fallback

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2f.AC4.1 | integration | `tests/orchestrator_test.rs` | Set `max_concurrent_workers=1`. Spawn a worker. Verify `project_path` is the original project directory (no worktree created). |

**Implementation tasks:** Phase 2f, Tasks 1-3.

---

## Phase 2g: CLI Updates

### P2g.AC1: Updated `run` command uses orchestrator

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2g.AC1.1 | human | N/A | See Human Verification table below. |
| P2g.AC1.2 | human | N/A | See Human Verification table below. |
| P2g.AC1.3 | human | N/A | See Human Verification table below. |
| P2g.AC1.4 | human | N/A | See Human Verification table below. |
| P2g.AC1.5 | human | N/A | See Human Verification table below. |

### P2g.AC2: Status command shows active workers and progress

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2g.AC2.1 | human | N/A | See Human Verification table below. |
| P2g.AC2.2 | human | N/A | See Human Verification table below. |
| P2g.AC2.3 | human | N/A | See Human Verification table below. |

### P2g.AC3: Graceful shutdown

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P2g.AC3.1 | integration | `tests/orchestrator_test.rs` | Create orchestrator with a CancellationToken. Cancel the token. Verify orchestrator stops, cancels all active workers, and saves session state. |
| P2g.AC3.2 | integration | `tests/orchestrator_test.rs` | After shutdown, verify InProgress tasks are recoverable. Combined with P2d.AC14.2 (recovery test). |

**Implementation tasks:** Phase 2g, Tasks 1-3.

---

## Human Verification Required

The following acceptance criteria cannot be fully automated because they depend on CLI output formatting, user-facing presentation, or live LLM interaction requiring API keys.

| AC | Phase | Reason | Verification Approach |
|----|-------|--------|----------------------|
| P2f.AC1.3 | 2f | Goal completion notification is a presentation concern. Branch creation and merge are tested automatically. | Run `cargo run -- run "simple goal" --workers 2`. After completion, verify CLI output includes the goal branch name (e.g., `rustagent/ra-XXXX`) and instructions to review/merge. |
| P2g.AC1.1 | 2g | End-to-end CLI `run` command using orchestrator requires a live LLM API key. | Set `ANTHROPIC_API_KEY`. Run `cargo run -- project add test-proj .` then `cargo run -- run "Create a hello world program" --workers 2`. Verify: planner creates tasks, workers spawn, tasks complete. Check logs at `RUST_LOG=rustagent=debug`. |
| P2g.AC1.2 | 2g | Full planner -> task creation -> scheduling -> worker completion flow with live LLM. | Verified as part of P2g.AC1.1 manual test. Check logs for planner spawning, task creation messages, and scheduling decisions. |
| P2g.AC1.3 | 2g | `--workers 1` flag behavior with live LLM. | Run `cargo run -- run "simple task" --workers 1`. Verify: no worktrees created, single worker executes sequentially (check logs and absence of `.rustagent/worktrees/`). |
| P2g.AC1.4 | 2g | `--workers N` flag sets max_concurrent_workers. | Run `cargo run -- run --help`. Verify `--workers` flag shown with default 4. Run with `--workers 2` and verify debug logs show max_concurrent_workers=2. |
| P2g.AC1.5 | 2g | `--review` flag enables review_required. | Run `cargo run -- run --help`. Verify `--review` flag shown. Run with `--review` and verify debug logs show review_required=true. |
| P2g.AC2.1 | 2g | Status display is presentation-level. Underlying data queries are tested in integration tests. | After running a goal, execute `cargo run -- status`. Verify output shows the current goal, orchestrator-related state, and active worker count. |
| P2g.AC2.2 | 2g | Task progress display is presentation-level. | Run `cargo run -- status` during or after a goal. Verify output shows completed/total counts and breakdown by status. |
| P2g.AC2.3 | 2g | Token usage display is presentation-level. | Run `cargo run -- status` after worker completion. Verify output includes "Token Usage:" line with cumulative token count. |

---

## Test File Summary

| Test File | Phase | Acceptance Criteria Covered |
|-----------|-------|-----------------------------|
| `tests/message_test.rs` | 2a | P2a.AC1.1-3, P2a.AC2.1-4, P2a.AC3.1-5 |
| `tests/work_package_test.rs` | 2b | P2b.AC1.1-3, P2b.AC2.1-6, P2b.AC3.1-2, P2b.AC4.1-3 |
| `tests/orchestrator_test.rs` | 2c, 2d, 2f, 2g | P2c.AC1-3, P2d.AC1-12, P2f.AC2.3, P2f.AC4.1, P2g.AC3.1-2 |
| `tests/orchestrator_e2e_test.rs` | 2d | P2d.AC13.1-3, P2d.AC14.1-2 |
| `tests/agent_tools_test.rs` | 2e | P2e.AC1.1-4, P2e.AC2.1-3, P2e.AC3.1-2, P2e.AC4.1-2 |
| `tests/worktree_test.rs` | 2f | P2f.AC1.1-2, P2f.AC2.1-2, P2f.AC3.1-3 |
| `tests/agent_types_test.rs` | 2d | P2d.AC6.3 |

---

## Coverage Audit

**Total acceptance criteria:** 103

- Phase 2a: 12
- Phase 2b: 14
- Phase 2c: 8 (2 deferred to Phase 2d)
- Phase 2d: 38
- Phase 2e: 11
- Phase 2f: 10
- Phase 2g: 10

**Automated test coverage:** 91 criteria (88%)

**Human verification only:** 10 criteria (10%)

**Deferred (tested in later phase):** 2 criteria (2%) -- P2c.AC4.1 (covered by P2d.AC5.1), P2c.AC4.2 (covered by P2d.AC5.2)

All 103 acceptance criteria are mapped to either an automated test, a documented human verification procedure, or identified as deferred to a specific later phase with explicit traceability.

---

## Cross-Phase Dependencies

| Criterion | Defined In | Tested In | Rationale |
|-----------|------------|-----------|-----------|
| P2c.AC4.1 | Phase 2c | Phase 2d (as P2d.AC5.1) | Recovery handler is `todo!()` in 2c; implemented in 2d |
| P2c.AC4.2 | Phase 2c | Phase 2d (as P2d.AC5.2) | Same as above |
| P2d.AC6.3 | Phase 2d | `tests/agent_types_test.rs` | Modifies Phase 1d's AgentOutcome type |
| P2g.AC3.2 | Phase 2g | Phase 2d (via P2d.AC14.2) | Recovery after shutdown uses same mechanism as recovery after crash |
