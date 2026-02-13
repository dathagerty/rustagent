# V2 Phase 5 - Agent Error Recovery & Task Reassignment

**Goal:** Enhance the orchestrator's error recovery to inject previous-attempt context into retried workers, cascade failure blocking to downstream tasks, and unblock tasks when a blocker resolves.

**Architecture:** The orchestrator already has core retry logic (`handle_task_retry_or_fail` at `orchestrator.rs:1058-1141`) with retry count tracking and observation node creation on final failure. This phase adds three missing pieces: (1) when spawning a retry worker, populate `AgentContext.previous_attempt` with the failed attempt's error so the new worker can learn from it; (2) when a task fails permanently, find downstream `DependsOn` tasks and mark them Blocked; (3) in `handle_scheduling`, check for Blocked tasks whose blockers have been resolved and unblock them.

**Tech Stack:** Rust (uses existing GraphStore trait, EdgeType::DependsOn, no new dependencies)

**Scope:** 1 of 6 phases from original design (Phase 5, item 6)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC11: Previous Attempt Context
- **v2-phase5.AC11.1 Success:** When a task is retried, the new worker's `AgentContext.previous_attempt` contains the previous failure's error description
- **v2-phase5.AC11.2 Success:** On the first attempt (no retries), `previous_attempt` is `None`

### v2-phase5.AC12: Failure Cascading
- **v2-phase5.AC12.1 Success:** When a task is permanently failed (retries exhausted), downstream tasks linked via `DependsOn` edges are marked Blocked with a `blocked_reason` referencing the failed task
- **v2-phase5.AC12.2 Success:** Tasks not dependent on the failed task are unaffected

### v2-phase5.AC13: Blocked Task Recovery
- **v2-phase5.AC13.1 Success:** During scheduling, tasks that are Blocked but whose blocker task has since been completed are transitioned back to Ready
- **v2-phase5.AC13.2 Success:** Tasks whose blocker is still Failed/Blocked remain Blocked

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Inject previous attempt context on retry

**Verifies:** v2-phase5.AC11.1, v2-phase5.AC11.2

**Files:**
- Modify: `src/agent/orchestrator.rs:1057-1095` (enhance retry path in `handle_task_retry_or_fail`)
- Modify: `src/agent/orchestrator.rs:797-806` (enhance `spawn_worker_with_id` to accept and forward `previous_attempt`)

**Implementation:**

1. In `handle_task_retry_or_fail`, when retrying (retry_count < max), store the failure error in the task's metadata under key `"previous_attempt"`:
```rust
metadata.insert("previous_attempt".to_string(), error.to_string());
```

2. In `spawn_worker_with_id`, when building the `AgentContext`, read `"previous_attempt"` from the task node's metadata:
```rust
let previous_attempt = task_nodes
    .first()
    .and_then(|t| t.metadata.get("previous_attempt").cloned());

let ctx = AgentContext {
    // ... existing fields ...
    previous_attempt,
    dependency_statuses: vec![], // populated in Phase 2
};
```

This requires Phase 2 (which adds `previous_attempt` and `dependency_statuses` fields to `AgentContext`) to be implemented first. If Phase 2 is not yet done, this task should use `None` and `vec![]` placeholders and update when Phase 2 lands.

**Testing:**
Tests must verify:
- v2-phase5.AC11.1: After a retry, the re-spawned AgentContext has `previous_attempt = Some("the error")`
- v2-phase5.AC11.2: On first attempt, `previous_attempt` is `None`

Test in `tests/orchestrator_test.rs` by setting up a task with `retry_count` metadata and verifying the context construction.

**Verification:**
Run: `cargo test orchestrator`
Expected: All tests pass

**Commit:** `feat(orchestrator): inject previous attempt context on task retry`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Cascade failure to downstream dependent tasks

**Verifies:** v2-phase5.AC12.1, v2-phase5.AC12.2

**Files:**
- Modify: `src/agent/orchestrator.rs:1095-1141` (add cascade after marking task Failed)

**Implementation:**

After marking a task as Failed and creating the observation node (line ~1129), add a call to cascade the failure to dependent tasks:

```rust
// Cascade failure: find downstream tasks that DependsOn this failed task
self.cascade_block_to_dependents(task_id).await?;
```

Implement `cascade_block_to_dependents` as a new method on `Orchestrator`:

```rust
/// Mark all tasks that directly depend on `blocker_id` as Blocked.
/// Stores the blocker task ID in each blocked task's metadata under key
/// `"blocker_task_id"` for reliable lookup during unblock checks.
async fn cascade_block_to_dependents(&self, blocker_id: &str) -> Result<()> {
    // DependsOn edge direction: if B DependsOn A, edge is from=B, to=A.
    // So get_edges(A, Incoming) finds edges where to_node=A, returning
    // the related from_node (B) — i.e., all tasks that depend on A.
    let edges = self.graph_store
        .get_edges(blocker_id, EdgeDirection::Incoming)
        .await?;

    let reason = format!("blocked by failed task {}", blocker_id);

    for (edge, node) in edges {
        if edge.edge_type == EdgeType::DependsOn
            && node.node_type == NodeType::Task
            && !matches!(node.status, NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Cancelled)
        {
            // Store blocker ID in metadata for reliable lookup during unblock
            let mut metadata = node.metadata.clone();
            metadata.insert("blocker_task_id".to_string(), blocker_id.to_string());

            self.graph_store
                .update_node(
                    &node.id,
                    Some(NodeStatus::Blocked),
                    None,                // title unchanged
                    None,                // description unchanged
                    Some(&reason),       // blocked_reason
                    Some(&metadata),     // metadata with blocker_task_id
                )
                .await?;

            tracing::info!(
                task = %node.id,
                blocker = %blocker_id,
                "Task blocked due to dependency failure"
            );
        }
    }

    Ok(())
}
```

Note: `DependsOn` edge direction — if task B `DependsOn` task A, the edge is `from=B, to=A`. When A fails, `get_edges(A, Incoming)` returns edges where `to_node = A`, with the related node being `from_node` (B). Verify this by checking `get_edges` semantics in `store.rs:713-749`.

**Testing:**
Tests must verify:
- v2-phase5.AC12.1: After task A fails permanently, task B (which DependsOn A) becomes Blocked with reason mentioning A
- v2-phase5.AC12.2: Task C (no dependency on A) remains unaffected

**Verification:**
Run: `cargo test orchestrator`
Expected: All tests pass

**Commit:** `feat(orchestrator): cascade failure blocking to dependent tasks`
<!-- END_TASK_2 -->

<!-- END_SUBCOMPONENT_A -->

<!-- START_SUBCOMPONENT_B (tasks 3-4) -->

<!-- START_TASK_3 -->
### Task 3: Unblock tasks when blocker resolves

**Verifies:** v2-phase5.AC13.1, v2-phase5.AC13.2

**Files:**
- Modify: `src/agent/orchestrator.rs:418-538` (add unblock check in `handle_scheduling`)

**Implementation:**

At the start of `handle_scheduling`, before querying ready tasks, add a pass that checks Blocked tasks:

```rust
// Check for blocked tasks that can be unblocked
self.try_unblock_tasks(&goal_id).await?;
```

Implement `try_unblock_tasks`:

```rust
/// Check all Blocked tasks under a goal and unblock any whose blockers have resolved.
/// Uses `metadata["blocker_task_id"]` (set by `cascade_block_to_dependents`) to
/// reliably identify which task is the blocker — no string parsing of blocked_reason.
async fn try_unblock_tasks(&self, goal_id: &str) -> Result<()> {
    let blocked_tasks = self.graph_store
        .query_nodes(&NodeQuery {
            node_type: Some(NodeType::Task),
            status: Some(NodeStatus::Blocked),
            project_id: Some(self.project_id.clone()),
            parent_id: None,
            query: None,
        })
        .await?;

    for task in &blocked_tasks {
        // Look up blocker task ID from metadata (set during cascade)
        if let Some(blocker_id) = task.metadata.get("blocker_task_id") {
            if let Some(blocker) = self.graph_store.get_node(blocker_id).await? {
                // If the blocker has been retried and is now completed, unblock
                if blocker.status == NodeStatus::Completed {
                    // Remove blocker_task_id from metadata when unblocking
                    let mut metadata = task.metadata.clone();
                    metadata.remove("blocker_task_id");

                    self.graph_store
                        .update_node(
                            &task.id,
                            Some(NodeStatus::Ready),
                            None,                // title unchanged
                            None,                // description unchanged
                            None,                // clear blocked_reason
                            Some(&metadata),     // metadata with blocker_task_id removed
                        )
                        .await?;

                    tracing::info!(
                        task = %task.id,
                        blocker = %blocker_id,
                        "Task unblocked: dependency resolved"
                    );
                }
            }
        }
    }

    Ok(())
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC13.1: A Blocked task referencing a failed task gets unblocked when that task later completes
- v2-phase5.AC13.2: A Blocked task whose blocker is still Failed stays Blocked

**Verification:**
Run: `cargo test orchestrator`
Expected: All tests pass

**Commit:** `feat(orchestrator): unblock tasks when blocker dependency resolves`
<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Integration test for full retry-cascade-unblock lifecycle

**Verifies:** v2-phase5.AC11.1, v2-phase5.AC12.1, v2-phase5.AC13.1

**Files:**
- Modify: `tests/orchestrator_test.rs` (add integration test)

**Implementation:**

Write a test that exercises the full lifecycle:
1. Create goal with task A and task B, where B DependsOn A
2. Simulate A failing (call `handle_task_retry_or_fail` with an error)
3. Verify A is reset to Ready with `previous_attempt` in metadata (retry)
4. Simulate A failing again (exceeding max_retries)
5. Verify A is marked Failed and an Observation node is created
6. Verify B is marked Blocked with reason referencing A
7. Manually complete A (simulate external fix)
8. Call `handle_scheduling` (which calls `try_unblock_tasks`)
9. Verify B is now Ready

Use the existing test helpers from `tests/common/mod.rs` (MockGraphStore or real SQLite).

**Testing:**
This IS the test.

**Verification:**
Run: `cargo test orchestrator`
Expected: All tests pass

**Commit:** `test(orchestrator): integration test for retry-cascade-unblock lifecycle`
<!-- END_TASK_4 -->

<!-- END_SUBCOMPONENT_B -->
