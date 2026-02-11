# Rustagent V2 Phase 2g: CLI Updates

**Goal:** Update the CLI to use the orchestrator for multi-agent execution, add a `status` command showing active workers and task progress, and add orchestrator config options as CLI flags.

**Architecture:** The `run` command currently spawns a single AgentRuntime directly. This phase updates it to use the Orchestrator, which handles goal creation, planning, scheduling, and worker management. The `status` command shows real-time information about the orchestrator's state, active workers, and task progress. Single-agent mode is preserved when `--workers 1` is specified.

**Tech Stack:** Rust (edition 2024), clap 4.5 (derive), tokio 1.43

**Scope:** Phase 7 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2g.AC1: Updated `run` command uses orchestrator
- **P2g.AC1.1 Success:** `rustagent run "goal"` creates an Orchestrator and runs it instead of directly creating an AgentRuntime
- **P2g.AC1.2 Success:** Orchestrator spawns a planner → planner creates tasks → orchestrator schedules workers
- **P2g.AC1.3 Success:** `--workers 1` flag falls back to single-worker mode (no worktrees, sequential execution)
- **P2g.AC1.4 Success:** `--workers N` flag sets max_concurrent_workers on OrchestratorConfig
- **P2g.AC1.5 Success:** `--review` flag enables review_required on OrchestratorConfig

### P2g.AC2: Status command shows active workers and progress
- **P2g.AC2.1 Success:** `rustagent status` shows the current goal, orchestrator state, and active worker count
- **P2g.AC2.2 Success:** `rustagent status` shows task progress (completed/total, breakdown by status)
- **P2g.AC2.3 Success:** `rustagent status` shows cumulative token usage for the active goal

### P2g.AC3: Graceful shutdown
- **P2g.AC3.1 Success:** Ctrl+C during `rustagent run` triggers graceful shutdown: cancel all workers, save session state, generate handoff notes
- **P2g.AC3.2 Success:** After graceful shutdown, re-running `rustagent run` with the same project resumes from where it left off (recovery)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Update `run` command to use Orchestrator

**Verifies:** P2g.AC1.1, P2g.AC1.3, P2g.AC1.4, P2g.AC1.5

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — update Run command and add orchestrator flags
- Test: manual verification via cargo build + cargo run

**Implementation:**

Update the `Run` variant in the `Commands` enum to add orchestrator options:

```rust
/// Execute a goal with an agent
Run {
    /// Goal description
    goal: String,
    /// Agent profile to use (for single-agent mode or default worker profile)
    #[arg(long, default_value = "coder")]
    profile: String,
    /// Maximum number of concurrent workers (default: 4, 1 = single-agent mode)
    #[arg(long, default_value = "4")]
    workers: usize,
    /// Require code review after each worker completes
    #[arg(long)]
    review: bool,
    /// Maximum token budget per goal (optional)
    #[arg(long)]
    max_tokens: Option<usize>,
},
```

Remove the deprecated `max_iterations` flag (replaced by `workers` and OrchestratorConfig.worker_turn_limit).

Update the `Commands::Run` match arm:

1. Open database, resolve project (existing code)
2. Create GraphStore, SessionStore (existing code)
3. Create LLM client via factory (existing code)
4. Create SecurityValidator and PermissionHandler (existing code)
5. Create TokioMessageBus
6. Build OrchestratorConfig from CLI flags:
   ```rust
   let orch_config = OrchestratorConfig {
       max_concurrent_workers: workers,
       review_required: review,
       max_tokens_per_goal: max_tokens,
       ..OrchestratorConfig::default()
   };
   ```
7. Create Orchestrator
8. Run orchestrator: `orchestrator.run(&goal, &project.id).await?`
9. Print results

For backwards compatibility: when `workers == 1`, the orchestrator uses single-agent mode (no worktrees, sequential task execution). The orchestrator handles this internally based on max_concurrent_workers.

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- run --help`
Expected: Shows updated flags including --workers, --review, --max-tokens

**Commit:** `feat(cli): update run command to use orchestrator with multi-agent support`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Enhance `status` command with worker and progress info

**Verifies:** P2g.AC2.1, P2g.AC2.2, P2g.AC2.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — enhance Status command handler

**Implementation:**

The current `Status` command shows basic project info. Enhance it to also show orchestration state when available.

The status display reads from the database (no connection to a running orchestrator needed — all state is in SQLite):

1. Find the latest session for the current project
2. If no active session: show "No active goal"
3. If active session exists:
   - Show goal title and ID
   - Query all task nodes under the goal
   - Count by status: Ready, Claimed, InProgress, Completed, Blocked, Failed
   - Show tasks assigned to agents (simulates "active workers")
   - Query worker_conversations for token totals

Output format:
```
Project: my-api (ra-a3f8)
Goal: Implement authentication system (ra-b2c1)
Session: sess-12345678 (started 2h ago)

Task Progress:
  Completed: 5/12
  In Progress: 2
  Ready: 3
  Blocked: 1
  Failed: 1

Active Workers:
  worker-1 (coder): Working on ra-b2c1.3 "Add login endpoint"
  worker-2 (tester): Working on ra-b2c1.5 "Write auth tests"

Token Usage: 45,230 tokens
```

**Testing:**

This is primarily a display change. Verification is operational:

Run: `cargo build`
Expected: Compiles cleanly

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

**Commit:** `feat(cli): enhance status command with worker and task progress display`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Implement graceful shutdown with Ctrl+C handling

**Verifies:** P2g.AC3.1, P2g.AC3.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add signal handling around orchestrator run
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — add shutdown method
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add shutdown test

**Implementation:**

**In main.rs**, wrap the orchestrator run with a Ctrl+C handler:

```rust
// In the Run command handler:
let shutdown_token = CancellationToken::new();
let shutdown_clone = shutdown_token.clone();

tokio::spawn(async move {
    tokio::signal::ctrl_c().await.ok();
    println!("\nGraceful shutdown initiated...");
    shutdown_clone.cancel();
});

match orchestrator.run_with_shutdown(&goal, &project.id, shutdown_token).await {
    Ok(()) => println!("Goal completed successfully."),
    Err(e) if e.to_string().contains("shutdown") => {
        println!("Session saved. Re-run to resume.");
    }
    Err(e) => return Err(e),
}
```

**In orchestrator.rs**, add a `run_with_shutdown` method:

```rust
pub async fn run_with_shutdown(
    &mut self,
    goal_description: &str,
    project_id: &str,
    shutdown_token: CancellationToken,
) -> Result<()> {
    loop {
        // Check for shutdown request before each state transition
        if shutdown_token.is_cancelled() {
            return self.handle_graceful_shutdown().await;
        }

        // ... same state machine loop as run() ...
    }
}

async fn handle_graceful_shutdown(&mut self) -> Result<()> {
    // 1. Cancel all active workers via their CancellationTokens
    for (_, handle) in &self.active_workers {
        handle.cancel_token.cancel();
    }

    // 2. Wait for workers to finish (with timeout)
    // Workers should detect cancellation and wrap up quickly

    // 3. Generate handoff notes from current state
    // 4. End the session with handoff notes
    // 5. Return a shutdown error so the caller knows this was interrupted

    anyhow::bail!("shutdown: session saved for recovery")
}
```

**Testing:**

- P2g.AC3.1: Create orchestrator with a shutdown token, cancel the token → orchestrator stops and saves state
- P2g.AC3.2: After shutdown, tasks that were InProgress are recoverable (tested by recovery logic in Phase 2d)

**Verification:**

Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(cli): graceful shutdown with Ctrl+C handling and session recovery`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
