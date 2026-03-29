# Rustagent V2 Phase 2f: Git Worktree Integration

**Goal:** Implement git worktree-based isolation for parallel workers. Each work package gets its own git worktree branched from a goal branch, providing true filesystem isolation. After worker completion, worktree branches are merged into the goal branch.

**Architecture:** Parallel workers operate in separate git worktrees to prevent filesystem conflicts. The lifecycle: goal start → create goal branch → per-work-package worktrees on sub-branches → workers write in worktrees → merge back into goal branch → cleanup. Workers never touch the user's main worktree. Since work packages have non-overlapping file scopes (enforced by FileOwnershipMap), merges are always clean.

**Tech Stack:** Rust (edition 2024), tokio 1.43, std::process::Command (for git CLI calls)

**Scope:** Phase 6 of 7 from the v2 Phase 2 architecture (Multi-Agent Orchestration)

**Codebase verified:** 2026-02-09

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P2f.AC1: Goal branch management
- **P2f.AC1.1 Success:** On goal start, a goal branch (`rustagent/<goal-id>`) is created from the current HEAD
- **P2f.AC1.2 Success:** If the goal branch already exists (recovery), it is reused without error
- **P2f.AC1.3 Success:** On goal completion, the user is informed about the goal branch (not auto-merged into main)

### P2f.AC2: Worktree creation for work packages
- **P2f.AC2.1 Success:** For each work package, a git worktree is created at a conventional path under `.git/worktrees/`
- **P2f.AC2.2 Success:** Each worktree is on its own branch (`rustagent/<goal-id>/wp-<id>`) forked from the goal branch
- **P2f.AC2.3 Success:** The worktree path is included in the worker's AgentContext.project_path so all file operations are rooted there

### P2f.AC3: Worktree merge and cleanup
- **P2f.AC3.1 Success:** On worker completion, the work package branch is merged into the goal branch
- **P2f.AC3.2 Success:** After successful merge, the worktree and work package branch are removed
- **P2f.AC3.3 Failure:** If merge fails (unexpected conflict), the error is reported and the worktree is preserved for manual resolution

### P2f.AC4: Single-agent fallback
- **P2f.AC4.1 Success:** When max_concurrent_workers=1, worktree isolation is skipped — workers operate directly in the project directory (same as Phase 1d behavior)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create worktree module with git operations

**Verifies:** P2f.AC1.1, P2f.AC1.2, P2f.AC2.1, P2f.AC2.2

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/worktree.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/mod.rs` — add `pub mod worktree;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/worktree_test.rs` (integration)

**Implementation:**

`src/agent/worktree.rs`:

All git operations use `std::process::Command` (synchronous, wrapped in `tokio::task::spawn_blocking` for async context). This follows the architecture's "no dedicated git tool" approach — git interaction through shell commands.

```rust
use anyhow::{Result, anyhow, Context};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages git worktrees for parallel worker isolation
pub struct WorktreeManager {
    project_path: PathBuf,
}

impl WorktreeManager {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    /// Create the goal branch from current HEAD. If branch exists, reuse it.
    pub fn create_goal_branch(&self, goal_id: &str) -> Result<String> {
        let branch_name = format!("rustagent/{}", goal_id);
        // git branch <name> HEAD (ignore error if already exists)
        let output = Command::new("git")
            .args(["branch", &branch_name, "HEAD"])
            .current_dir(&self.project_path)
            .output()
            .context("failed to create goal branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already exists") {
                // Branch already exists (recovery case) — this is OK
                return Ok(branch_name);
            }
            return Err(anyhow!("failed to create goal branch: {}", stderr));
        }
        Ok(branch_name)
    }

    /// Create a worktree for a work package.
    /// Returns the path to the worktree directory.
    pub fn create_worktree(&self, goal_id: &str, work_package_id: &str) -> Result<PathBuf> {
        let branch_name = format!("rustagent/{}/wp-{}", goal_id, work_package_id);
        let goal_branch = format!("rustagent/{}", goal_id);

        // Worktree path: <project>/.git/worktrees/ is managed by git,
        // actual worktree dirs go in a sibling directory
        let worktree_path = self.project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-wp-{}", goal_id, work_package_id));

        // git worktree add -b <branch> <path> <start-point>
        let output = Command::new("git")
            .args([
                "worktree", "add",
                "-b", &branch_name,
                worktree_path.to_str().ok_or_else(|| anyhow!("invalid worktree path"))?,
                &goal_branch,
            ])
            .current_dir(&self.project_path)
            .output()
            .context("failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("failed to create worktree: {}", stderr));
        }

        Ok(worktree_path)
    }

    /// Merge a work package branch into the goal branch.
    ///
    /// Uses a dedicated merge worktree to avoid depending on the main worktree's
    /// current branch. The merge worktree checks out the goal branch, merges the
    /// work package branch, then cleans up. This avoids race conditions where the
    /// main worktree might be on a different branch.
    pub fn merge_work_package(&self, goal_id: &str, work_package_id: &str) -> Result<()> {
        let branch_name = format!("rustagent/{}/wp-{}", goal_id, work_package_id);
        let goal_branch = format!("rustagent/{}", goal_id);

        // Create a temporary merge worktree on the goal branch
        let merge_path = self.project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-merge-tmp", goal_id));

        // If the merge worktree already exists (from a previous failed merge), remove it first
        if merge_path.exists() {
            let _ = Command::new("git")
                .args(["worktree", "remove", "--force",
                    merge_path.to_str().unwrap_or_default()])
                .current_dir(&self.project_path)
                .output();
        }

        // Create temporary worktree on goal branch (no new branch — use existing)
        let output = Command::new("git")
            .args([
                "worktree", "add",
                merge_path.to_str().ok_or_else(|| anyhow!("invalid merge path"))?,
                &goal_branch,
            ])
            .current_dir(&self.project_path)
            .output()
            .context("failed to create merge worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("failed to create merge worktree: {}", stderr));
        }

        // Merge the work package branch into the goal branch (within the merge worktree)
        let merge_result = Command::new("git")
            .args(["merge", "--no-ff", "-m",
                &format!("rustagent: merge work package wp-{}", work_package_id),
                &branch_name
            ])
            .current_dir(&merge_path)
            .output()
            .context("failed to merge work package branch")?;

        // Clean up the merge worktree regardless of outcome
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force",
                merge_path.to_str().unwrap_or_default()])
            .current_dir(&self.project_path)
            .output();

        if !merge_result.status.success() {
            let stderr = String::from_utf8_lossy(&merge_result.stderr);
            return Err(anyhow!("merge conflict in work package branch: {}", stderr));
        }

        Ok(())
    }

    /// Remove a worktree and its branch after successful merge.
    pub fn cleanup_worktree(&self, goal_id: &str, work_package_id: &str) -> Result<()> {
        let branch_name = format!("rustagent/{}/wp-{}", goal_id, work_package_id);
        let worktree_path = self.project_path
            .join(".rustagent")
            .join("worktrees")
            .join(format!("{}-wp-{}", goal_id, work_package_id));

        // Remove worktree
        let _ = Command::new("git")
            .args(["worktree", "remove", "--force",
                worktree_path.to_str().unwrap_or_default()])
            .current_dir(&self.project_path)
            .output();

        // Delete branch
        let _ = Command::new("git")
            .args(["branch", "-d", &branch_name])
            .current_dir(&self.project_path)
            .output();

        Ok(())
    }
}
```

**Testing:**

Tests use `tempfile::TempDir` and initialize a git repo for each test:

```rust
fn init_test_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    Command::new("git").args(["init"]).current_dir(&path).output().unwrap();
    Command::new("git").args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&path).output().unwrap();
    (dir, path)
}
```

- P2f.AC1.1: create_goal_branch → branch exists in git branch list
- P2f.AC1.2: create_goal_branch twice → no error on second call
- P2f.AC2.1: create_worktree → directory exists at the worktree path
- P2f.AC2.2: create_worktree → git branch list shows the wp branch

**Verification:**

Run: `cargo test worktree_test`
Expected: All tests pass

**Commit:** `feat(agent): WorktreeManager for git worktree-based worker isolation`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Integrate worktrees with orchestrator worker spawning

**Verifies:** P2f.AC2.3, P2f.AC3.1, P2f.AC3.2, P2f.AC3.3, P2f.AC4.1

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/orchestrator.rs` — integrate WorktreeManager into spawn_worker and worker completion
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/orchestrator_test.rs` — add worktree integration tests

**Implementation:**

Add `WorktreeManager` to the Orchestrator struct (optional — None for single-agent mode):

```rust
pub struct Orchestrator {
    // ... existing fields ...
    worktree_manager: Option<WorktreeManager>,
}
```

Initialize in `new()`: if `config.max_concurrent_workers > 1`, create a WorktreeManager.

**In handle_loading** (after creating/loading goal):
- If worktree_manager is Some, call `create_goal_branch(goal_id)`

**In spawn_worker:**
- If worktree_manager is Some:
  - Call `create_worktree(goal_id, work_package_id)` to get worktree_path
  - Set `AgentContext.project_path = worktree_path`
- If worktree_manager is None (single-agent mode):
  - Set `AgentContext.project_path = self.project_path` (existing behavior)

**On worker completion:**
- If worktree_manager is Some:
  - If worker succeeded: call `merge_work_package()` then `cleanup_worktree()`
  - If worker failed: log a warning but don't cleanup (preserves state for debugging)

**On goal completion (handle_completing):**
- Log the goal branch name so the user knows where the combined work is
- Do NOT auto-merge into main — the user decides

**Testing:**

Tests for worktree integration require a real git repo (use tempfile::TempDir):

- P2f.AC2.3: Spawn a worker with worktree manager → worker's context has worktree path as project_path
- P2f.AC4.1: Set max_concurrent_workers=1, spawn worker → project_path is the original project path (no worktree)
- P2f.AC3.1: Worker completes → work package branch is merged into goal branch (verify with git log)
- P2f.AC3.2: After merge, worktree directory is removed
- P2f.AC3.3: If merge fails (simulate with conflicting changes), error is returned and worktree preserved

For P2f.AC3.3, create a test where two branches modify the same file (bypassing FileOwnershipMap for testing purposes).

**Verification:**

Run: `cargo test worktree_test`
Run: `cargo test orchestrator_test`
Expected: All tests pass

**Commit:** `feat(agent): integrate git worktrees with orchestrator worker lifecycle`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add .rustagent/worktrees/ to .gitignore handling

**Verifies:** None (infrastructure)

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/worktree.rs` — add gitignore management

**Implementation:**

When creating the first worktree, ensure `.rustagent/worktrees/` is in `.gitignore`:

```rust
impl WorktreeManager {
    /// Ensure .rustagent/worktrees/ is gitignored
    pub fn ensure_gitignore(&self) -> Result<()> {
        let gitignore_path = self.project_path.join(".gitignore");
        let entry = ".rustagent/worktrees/";

        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            if content.contains(entry) {
                return Ok(()); // Already present
            }
            // Append
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&gitignore_path)?;
            use std::io::Write;
            writeln!(file, "\n# Rustagent worktrees (auto-generated)")?;
            writeln!(file, "{}", entry)?;
        } else {
            std::fs::write(&gitignore_path, format!("# Rustagent worktrees (auto-generated)\n{}\n", entry))?;
        }
        Ok(())
    }
}
```

Call `ensure_gitignore()` from `create_goal_branch()`.

**Verification:**

Run: `cargo test worktree_test`
Expected: All tests pass

**Commit:** `feat(agent): auto-manage .gitignore for worktree directories`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
