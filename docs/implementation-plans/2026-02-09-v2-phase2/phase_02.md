# Rustagent V2 Phase 2b: WorkPackage + File Ownership

**Goal:** Implement the WorkPackage type for grouping related tasks, the FileOwnershipMap for preventing concurrent file modification conflicts, the Complexity enum, and the WorkerHandle/WorkerState types used by the orchestrator to track active workers.

**Architecture:** Instead of assigning one task per worker (wasteful for small tasks), the orchestrator groups related tasks into work packages based on file overlap and dependency chains. The FileOwnershipMap ensures no two workers modify the same files simultaneously. WorkerHandle wraps the tokio JoinHandle + CancellationToken for lifecycle management.

**Tech Stack:** Rust (edition 2024), tokio 1.43, tokio-util 0.7 (CancellationToken), async-trait 0.1, chrono

**Scope:** Phase 2 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2b.AC1: WorkPackage type
- **P2b.AC1.1 Success:** WorkPackage struct has fields: id, task_ids, file_scope, profile, priority, estimated_complexity
- **P2b.AC1.2 Success:** Complexity enum has Small, Medium, Large variants
- **P2b.AC1.3 Success:** WorkPackage uses existing Priority enum from `graph::Priority`

### P2b.AC2: FileOwnershipMap
- **P2b.AC2.1 Success:** `acquire()` grants ownership of files to an agent when no conflicts exist
- **P2b.AC2.2 Failure:** `acquire()` returns error when any file is already owned by a different agent
- **P2b.AC2.3 Success:** `release()` frees all files owned by an agent
- **P2b.AC2.4 Success:** `can_write()` returns true for files owned by the querying agent
- **P2b.AC2.5 Success:** `can_write()` returns false for files owned by a different agent
- **P2b.AC2.6 Success:** `can_write()` returns true for files not owned by anyone (uncontested writes)

### P2b.AC3: WorkerHandle and WorkerState
- **P2b.AC3.1 Success:** WorkerHandle struct has fields: id, profile, work_package, state, join_handle, cancel_token, spawned_at, last_check_in
- **P2b.AC3.2 Success:** WorkerState enum has variants: Spawning, Initializing, Working, Reporting, Completed(AgentOutcome), Failed(String)

### P2b.AC4: Task grouping logic
- **P2b.AC4.1 Success:** Tasks modifying the same files are grouped into the same work package
- **P2b.AC4.2 Success:** Tasks with sequential dependencies (DependsOn edges) are grouped into the same work package
- **P2b.AC4.3 Success:** Independent tasks with separate file scopes produce separate work packages

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create work_package module with WorkPackage, Complexity, and FileOwnershipMap

**Verifies:** P2b.AC1.1, P2b.AC1.2, P2b.AC1.3, P2b.AC2.1, P2b.AC2.2, P2b.AC2.3, P2b.AC2.4, P2b.AC2.5, P2b.AC2.6

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/work_package.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/mod.rs` — add `pub mod work_package;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/work_package_test.rs` (integration)

**Implementation:**

`src/agent/work_package.rs`:

**Complexity enum:**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    Small,   // 1-2 file changes, straightforward
    Medium,  // Multiple files, some decision-making
    Large,   // Architectural changes, many files
}
```

**WorkPackage struct:**

```rust
use crate::graph::Priority;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WorkPackage {
    pub id: String,
    pub task_ids: Vec<String>,
    pub file_scope: Vec<PathBuf>,
    pub profile: String,
    pub priority: Priority,
    pub estimated_complexity: Complexity,
}
```

**FileOwnershipMap:**

```rust
use crate::agent::AgentId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Result, anyhow};

#[derive(Debug, Default)]
pub struct FileOwnershipMap {
    locks: HashMap<PathBuf, AgentId>,
}

impl FileOwnershipMap {
    pub fn new() -> Self {
        Self { locks: HashMap::new() }
    }

    /// Try to acquire ownership of files for an agent.
    /// Returns Err if any file is already owned by another agent.
    pub fn acquire(&mut self, agent_id: &AgentId, files: &[PathBuf]) -> Result<()> {
        // Pre-check: all files must be unowned or owned by this agent
        for file in files {
            if let Some(owner) = self.locks.get(file) {
                if owner != agent_id {
                    return Err(anyhow!(
                        "file {} is already owned by agent {}", file.display(), owner
                    ));
                }
            }
        }
        // All clear — acquire
        for file in files {
            self.locks.insert(file.clone(), agent_id.clone());
        }
        Ok(())
    }

    /// Release all files owned by an agent
    pub fn release(&mut self, agent_id: &AgentId) {
        self.locks.retain(|_, owner| owner != agent_id);
    }

    /// Check if a file write is permitted for an agent.
    /// Returns true if the file is unowned or owned by this agent.
    pub fn can_write(&self, agent_id: &AgentId, file: &Path) -> bool {
        match self.locks.get(file) {
            Some(owner) => owner == agent_id,
            None => true, // Unowned files are writable
        }
    }
}
```

**Testing:**

Tests in `tests/work_package_test.rs` using `#[test]` (FileOwnershipMap is synchronous):

- P2b.AC1.1: Construct a WorkPackage with all fields, assert fields are accessible
- P2b.AC1.2: Construct each Complexity variant
- P2b.AC2.1: Create map, acquire files for agent "a1", verify can_write returns true
- P2b.AC2.2: Acquire files for "a1", attempt acquire same files for "a2" — returns Err
- P2b.AC2.3: Acquire files for "a1", release "a1", then acquire same files for "a2" — succeeds
- P2b.AC2.4: Acquire "src/main.rs" for "a1", can_write("a1", "src/main.rs") returns true
- P2b.AC2.5: Acquire "src/main.rs" for "a1", can_write("a2", "src/main.rs") returns false
- P2b.AC2.6: Empty map, can_write("a1", "src/anything.rs") returns true

**Verification:**

Run: `cargo test work_package_test`
Expected: All tests pass

**Commit:** `feat(agent): WorkPackage type, Complexity enum, and FileOwnershipMap`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create WorkerHandle and WorkerState types

**Verifies:** P2b.AC3.1, P2b.AC3.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/work_package.rs` — add WorkerHandle and WorkerState
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/work_package_test.rs` — add tests

**Implementation:**

Add to `src/agent/work_package.rs`:

```rust
use crate::agent::AgentOutcome;
use chrono::{DateTime, Utc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// State of a worker during its lifecycle
#[derive(Debug, Clone)]
pub enum WorkerState {
    Spawning,
    Initializing,
    Working,
    Reporting,
    Completed(AgentOutcome),
    Failed(String),
}

/// Handle to a running worker, held by the orchestrator
pub struct WorkerHandle {
    pub id: AgentId,
    pub profile: String,
    pub work_package: WorkPackage,
    pub state: WorkerState,
    pub join_handle: JoinHandle<Result<AgentOutcome>>,
    pub cancel_token: CancellationToken,
    pub spawned_at: DateTime<Utc>,
    pub last_check_in: DateTime<Utc>,
}
```

Note: `WorkerHandle` cannot derive `Debug` because `JoinHandle` doesn't implement `Debug` in all cases. Implement `Debug` manually if needed, or skip it (the orchestrator logs state via `WorkerState` which is Debug).

**Testing:**

Tests in `tests/work_package_test.rs`:

- P2b.AC3.2: Construct each WorkerState variant, assert Debug formatting works
- P2b.AC3.1: Verify WorkerHandle fields compile (create one with a dummy JoinHandle from `tokio::spawn`, assert id and profile accessible)

Use `#[tokio::test]` for WorkerHandle test since it needs tokio::spawn.

**Verification:**

Run: `cargo test work_package_test`
Expected: All tests pass

**Commit:** `feat(agent): WorkerHandle and WorkerState types`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Implement task grouping logic

**Verifies:** P2b.AC4.1, P2b.AC4.2, P2b.AC4.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/work_package.rs` — add `group_tasks_into_packages` function
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/work_package_test.rs` — add tests

**Implementation:**

Add a function that takes a list of ready tasks (with their metadata and dependency edges) and produces work packages:

```rust
/// Input for task grouping: a ready task with its file scope metadata
#[derive(Debug, Clone)]
pub struct TaskForGrouping {
    pub task_id: String,
    pub file_scope: Vec<PathBuf>,
    pub profile: String,
    pub priority: Priority,
    pub depends_on: Vec<String>,  // IDs of tasks this depends on (within the ready set)
}

/// Group ready tasks into work packages based on file overlap and dependencies.
///
/// Grouping rules (from architecture):
/// 1. Tasks that modify the same files → same work package
/// 2. Tasks with sequential dependencies → same work package
/// 3. Independent tasks with separate file scopes → separate work packages
pub fn group_tasks_into_packages(tasks: Vec<TaskForGrouping>) -> Vec<WorkPackage>;
```

The implementation uses a union-find (disjoint set) approach:
1. Start with each task in its own group
2. For each pair of tasks that share any file in file_scope → merge groups
3. For each dependency edge (task A depends on task B, both in ready set) → merge groups
4. Build a WorkPackage from each group, combining file_scope (deduplicated), taking the highest priority, using the most common profile, and estimating complexity based on total file count

Work package IDs: `wp-{8 hex chars from uuid}`.

Complexity estimation: file_scope.len() <= 2 → Small, <= 6 → Medium, else Large.

**Testing:**

Tests in `tests/work_package_test.rs`:

- P2b.AC4.1: Two tasks sharing "src/main.rs" → grouped into 1 work package
- P2b.AC4.2: Task A depends on Task B (both ready) → grouped into 1 work package
- P2b.AC4.3: Two tasks with completely separate files and no dependencies → 2 work packages
- Work package ID format: Verify generated IDs start with "wp-" and have 8 hex characters (regex: `^wp-[0-9a-f]{8}$`)

**Verification:**

Run: `cargo test work_package_test`
Expected: All tests pass

**Commit:** `feat(agent): task grouping logic for work packages`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
