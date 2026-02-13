# V2 Phase 5 - ContextBuilder Enhancement

**Goal:** Enhance the ContextBuilder to implement the full context template from the design: dependency status, previous attempt outcomes, priority-based token budgeting, and overflow trimming.

**Architecture:** `AgentContext` gains two new fields: `previous_attempt` (optional outcome description from a prior failed run) and `dependency_statuses` (list of dependency nodes with completion status). `ContextBuilder::build_system_prompt` is updated to render these sections in the design's compact format. A new `ContextBudget` struct manages the priority-based token allocation and overflow trimming described in the architecture doc.

**Tech Stack:** Rust (no new dependencies — token counting is approximated as `text.len() / 4`)

**Scope:** 1 of 6 phases from original design (Phase 5, item 2)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC3: ContextBuilder Sections
- **v2-phase5.AC3.1 Success:** System prompt includes `[DEP:DONE]` lines for completed dependencies and `[DEP:PENDING]` lines for pending ones
- **v2-phase5.AC3.2 Success:** System prompt includes `[PREV_ATTEMPT]` section when a previous attempt outcome is present
- **v2-phase5.AC3.3 Success:** System prompt omits `## Previous Attempt` section entirely when no previous attempt exists

### v2-phase5.AC4: Token Budget
- **v2-phase5.AC4.1 Success:** When total context exceeds the token budget, lower-priority sections are trimmed (observations first, then AGENTS.md summaries, then decisions)
- **v2-phase5.AC4.2 Success:** Required sections (Role, Task, Rules) are never trimmed regardless of budget

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Add new fields to AgentContext

**Verifies:** v2-phase5.AC3.1, v2-phase5.AC3.2

**Files:**
- Modify: `src/agent/mod.rs:54-76` (add fields to `AgentContext` struct)

**Implementation:**

Add two new fields to `AgentContext`:

```rust
/// Previous attempt outcome description (if this is a retry)
pub previous_attempt: Option<String>,

/// Dependency statuses: (node_id, title, is_completed)
pub dependency_statuses: Vec<(String, String, bool)>,
```

Update the `Debug` impl to include the new fields. Update all call sites that construct `AgentContext` to pass default values (`None` and `vec![]`).

Call sites to update (verified via grep for `AgentContext {`):
- `src/agent/orchestrator.rs:798` (in `spawn_worker_with_id`)
- `src/context/mod.rs:422` (inline test)
- `tests/agent_runtime_test.rs:15` (in `make_test_context()`)
- `tests/agent_types_test.rs:76, 174` (two test functions)

**Testing:**
No dedicated tests needed — compiler will enforce all call sites are updated.

**Verification:**
Run: `cargo check`
Expected: Compiles without errors

**Commit:** `feat(agent): add previous_attempt and dependency_statuses to AgentContext`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Render dependency status and previous attempt in ContextBuilder

**Verifies:** v2-phase5.AC3.1, v2-phase5.AC3.2, v2-phase5.AC3.3

**Files:**
- Modify: `src/context/mod.rs:17-95` (update `build_system_prompt` method)

**Implementation:**

Add two new sections to `build_system_prompt`, inserted after the Task section and before Session Continuity:

1. **Dependency status** (after Task section, before Previous Attempt):
```
// After the task [CRITERIA] lines, add dependency lines:
for (id, title, is_done) in &ctx.dependency_statuses {
    if *is_done {
        prompt.push_str(&format!("[DEP:DONE] {} → {} (completed)\n", id, title));
    } else {
        prompt.push_str(&format!("[DEP:PENDING] {} → {} (pending)\n", id, title));
    }
}
```

2. **Previous attempt** (new section between dependencies and Session Continuity):
```
if let Some(prev) = &ctx.previous_attempt {
    prompt.push_str("\n## Previous Attempt\n");
    prompt.push_str(&format!("[PREV_ATTEMPT] {}\n\n", prev));
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC3.1: Prompt contains `[DEP:DONE]` and `[DEP:PENDING]` when dependencies are present
- v2-phase5.AC3.2: Prompt contains `## Previous Attempt` and `[PREV_ATTEMPT]` when previous_attempt is `Some`
- v2-phase5.AC3.3: Prompt does NOT contain `## Previous Attempt` when previous_attempt is `None`

**Verification:**
Run: `cargo test context`
Expected: All tests pass

**Commit:** `feat(context): render dependency status and previous attempt in system prompt`
<!-- END_TASK_2 -->

<!-- END_SUBCOMPONENT_A -->

<!-- START_SUBCOMPONENT_B (tasks 3-4) -->

<!-- START_TASK_3 -->
### Task 3: Implement ContextBudget for token-aware assembly

**Verifies:** v2-phase5.AC4.1, v2-phase5.AC4.2

**Files:**
- Modify: `src/context/mod.rs` (add `ContextBudget` struct and `build_system_prompt_with_budget` method)

**Implementation:**

Add a `ContextBudget` struct that manages priority-based token allocation:

```rust
pub struct ContextBudget {
    /// Total token budget for the system prompt (default: 4000)
    pub max_tokens: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self { max_tokens: 4000 }
    }
}
```

Add a new method `build_system_prompt_with_budget(ctx: &AgentContext, budget: &ContextBudget) -> String` that:

1. Builds required sections first (Role, Task + deps, Previous Attempt, Rules) — these are never trimmed. Previous Attempt is in the required set because an agent retrying without knowing why it failed defeats the purpose of the retry. The design's priority table lists it at priority 3, but since retries are rare and the content is short (a single error string), including it unconditionally is the right call.
2. Builds optional sections in priority order: Session Continuity, Active Decisions, Relevant Observations, Project Conventions
3. Estimates token count as `text.len() / 4` (rough approximation)
4. If the required + all optional sections fit within budget, return the full prompt
5. If over budget, trim from lowest priority up: drop Project Conventions, then Observations, then Decisions, then Session Continuity
6. The existing `build_system_prompt` method remains unchanged (no budget, includes everything) for backward compatibility

**Testing:**
Tests must verify:
- v2-phase5.AC4.1: With a very small budget (e.g., 200 tokens), only required sections appear; optional sections are trimmed
- v2-phase5.AC4.2: Required sections (Role, Task, Rules) are always present even with a tiny budget

**Verification:**
Run: `cargo test context`
Expected: All tests pass

**Commit:** `feat(context): add token budget-aware context assembly`
<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Wire budget-aware builder into AgentRuntime

**Verifies:** v2-phase5.AC4.1

**Files:**
- Modify: `src/agent/runtime.rs` (in `AgentRuntime::run`, use `build_system_prompt_with_budget` instead of `build_system_prompt`)

**Implementation:**

Update `AgentRuntime::run` to use `ContextBuilder::build_system_prompt_with_budget` with a default `ContextBudget`. The budget can later be made configurable via `RuntimeConfig` if needed, but for now use the default (4000 tokens).

```rust
let budget = ContextBudget::default();
let system_prompt = ContextBuilder::build_system_prompt_with_budget(&ctx, &budget);
```

**Testing:**
No new tests needed — existing agent_runtime_test.rs tests cover that the runtime starts correctly. The budget is tested in Task 3.

**Verification:**
Run: `cargo test agent_runtime`
Expected: All tests pass

**Commit:** `feat(runtime): use budget-aware context builder`
<!-- END_TASK_4 -->

<!-- END_SUBCOMPONENT_B -->
