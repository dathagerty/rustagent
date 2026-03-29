# V2 Phase 5 - Test Requirements

Generated from Acceptance Criteria across all 6 implementation phases.

## Summary

- **38 acceptance criteria** mapped to automated tests
- **8 test locations** (inline modules + integration test files)
- **1 human verification** (end-to-end smoke test)

---

## Phase 1: AGENTS.md Parser Enhancement

### v2-phase5.AC1: AGENTS.md Resolution

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC1.1 | Unit | `src/context/agents_md.rs` (inline `#[cfg(test)]`) | `resolve_agents_md` returns headings with correct line counts, ordered closest-to-file first |
| v2-phase5.AC1.2 | Unit | `src/context/agents_md.rs` (inline `#[cfg(test)]`) | Returned paths are relative to project root (no absolute path prefix) |
| v2-phase5.AC1.3 | Unit | `src/context/agents_md.rs` (inline `#[cfg(test)]`) | Deduplication works — same AGENTS.md not returned twice |

### v2-phase5.AC2: ReadAgentsMdTool Enhancement

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC2.1 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | Tool accepts directory path (e.g., `src/auth`) and reads AGENTS.md within it |
| v2-phase5.AC2.2 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | Tool still accepts full file path (`src/auth/AGENTS.md`) for backward compatibility |

---

## Phase 2: ContextBuilder Enhancement

### v2-phase5.AC3: ContextBuilder Sections

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC3.1 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | System prompt includes `[DEP:DONE]` for completed deps and `[DEP:PENDING]` for pending deps |
| v2-phase5.AC3.2 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | System prompt includes `[PREV_ATTEMPT]` section when previous attempt is present |
| v2-phase5.AC3.3 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | System prompt omits `## Previous Attempt` section entirely when no previous attempt exists |

### v2-phase5.AC4: Token Budget

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC4.1 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | With small budget, lower-priority sections are trimmed (observations first, then AGENTS.md, then decisions) |
| v2-phase5.AC4.2 | Unit | `src/context/mod.rs` (inline `#[cfg(test)]`) | Required sections (Role, Task, Rules) are never trimmed regardless of budget |

---

## Phase 3: Autonomy Levels & Approval Gates

### v2-phase5.AC5: Autonomy Level Types

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC5.1 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `AutonomyLevel::Full` has no active gates (empty vec) |
| v2-phase5.AC5.2 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `AutonomyLevel::Supervised` activates PlanReview, PreCommit, TaskComplete, GoalComplete |
| v2-phase5.AC5.3 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `AutonomyLevel::Gated` activates all 6 gates |
| v2-phase5.AC5.4 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `AutonomyLevel::default()` is `Supervised` |

### v2-phase5.AC6: Approval Request/Response

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC6.1 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `ApprovalRequest` can be constructed and serialized to JSON with all required fields |
| v2-phase5.AC6.2 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | All three `ApprovalResponse` variants serialize/deserialize correctly (round-trip) |

### v2-phase5.AC14: Gate Checking

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC14.1 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `GateChecker::new(Full).check_gate(PlanReview, ...)` returns `None` |
| v2-phase5.AC14.2 | Unit | `src/autonomy.rs` (inline `#[cfg(test)]`) | `GateChecker::new(Supervised).check_gate(PlanReview, ...)` returns `Some(ApprovalRequest)` with correct gate and fields |

---

## Phase 4: Security Scope Enforcement

### v2-phase5.AC7: Path Validation

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC7.1 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_path` allows path matching allowed pattern (e.g., `src/main.rs` matches `src/**`) |
| v2-phase5.AC7.2 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_path` denies path matching denied pattern even if also matching allowed pattern |
| v2-phase5.AC7.3 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_path` denies all write operations when `read_only` is true |
| v2-phase5.AC7.4 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_path` denies creating new files when `can_create_files` is false |
| v2-phase5.AC7.5 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | Wildcard `*` in allowed_paths matches everything |

Additional test: `src/**` matches `src/auth/handler.rs` (recursive directory matching with `matches_path_with`).

### v2-phase5.AC8: Command Validation

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC8.1 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_command` allows matching command (e.g., `cargo test` matches `cargo *`) |
| v2-phase5.AC8.2 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | `check_command` denies command not matching any allowed pattern |
| v2-phase5.AC8.3 | Unit | `src/security/scope.rs` (inline `#[cfg(test)]`) | Wildcard `*` in allowed_commands matches everything |

### Integration: Built-in Profile Scope Tests

| Test Type | Test Location | Description |
|-----------|---------------|-------------|
| Integration | `tests/security_scope_test.rs` | Planner/reviewer/researcher profiles deny writes (read_only), coder profile allows writes |

---

## Phase 5: Code Search Tool

### v2-phase5.AC9: Code Search Functionality

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC9.1 | Unit | `src/tools/search.rs` (inline `#[cfg(test)]`) | Search finds known pattern, result includes file path and line number |
| v2-phase5.AC9.2 | Unit | `src/tools/search.rs` (inline `#[cfg(test)]`) | File glob filter limits search to matching files |
| v2-phase5.AC9.3 | Unit | `src/tools/search.rs` (inline `#[cfg(test)]`) | Results capped at max_results |
| v2-phase5.AC9.4 | Unit | `src/tools/search.rs` (inline `#[cfg(test)]`) | Binary files skipped without error |

### v2-phase5.AC10: Code Search Registration

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC10.1 | Build check | `cargo check` | Search tool registered in V2 registry (compiler enforces new parameter) |
| v2-phase5.AC10.2 | Unit | `src/tools/search.rs` (inline `#[cfg(test)]`) | Tool's JSON schema describes pattern (required), file_glob, directory, max_results (optional) |

### Integration: Code Search Tests

| Test Type | Test Location | Description |
|-----------|---------------|-------------|
| Integration | `tests/code_search_test.rs` | End-to-end search with tempdir, glob filtering, max_results cap, no-match case |

---

## Phase 6: Agent Error Recovery & Task Reassignment

### v2-phase5.AC11: Previous Attempt Context

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC11.1 | Integration | `tests/orchestrator_test.rs` | After retry, re-spawned AgentContext has `previous_attempt = Some("the error")` |
| v2-phase5.AC11.2 | Integration | `tests/orchestrator_test.rs` | On first attempt, `previous_attempt` is `None` |

### v2-phase5.AC12: Failure Cascading

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC12.1 | Integration | `tests/orchestrator_test.rs` | After task A fails permanently, task B (DependsOn A) becomes Blocked with reason and `blocker_task_id` in metadata |
| v2-phase5.AC12.2 | Integration | `tests/orchestrator_test.rs` | Task C (no dependency on A) remains unaffected |

### v2-phase5.AC13: Blocked Task Recovery

| Criterion | Test Type | Test Location | Description |
|-----------|-----------|---------------|-------------|
| v2-phase5.AC13.1 | Integration | `tests/orchestrator_test.rs` | Blocked task with `blocker_task_id` metadata gets unblocked when blocker completes |
| v2-phase5.AC13.2 | Integration | `tests/orchestrator_test.rs` | Blocked task whose blocker is still Failed stays Blocked |

### Integration: Full Lifecycle Test

| Test Type | Test Location | Description |
|-----------|---------------|-------------|
| Integration | `tests/orchestrator_test.rs` | Full retry-cascade-unblock lifecycle: retry with previous_attempt -> permanent failure -> cascade block -> manual completion -> unblock |

---

## Human Verification

### End-to-End Smoke Test

**Justification:** The implementation phases build types, enforcement logic, and orchestrator extensions independently. A human should verify that an agent running with a `Gated` autonomy level and a restricted `SecurityScope` correctly receives context with all new sections (dependency status, previous attempt) and that the code search tool returns results when invoked by the agent. This requires a running LLM provider and is not feasible to automate in CI.

**Verification approach:**
1. Start the daemon with a test project
2. Create a goal with `--profile coder` and `--autonomy gated`
3. Verify that gate prompts appear at configured points (PlanReview, PreCommit, etc.)
4. Verify that the agent can use the `code_search` tool and receive results
5. Verify that SecurityScope enforcement blocks operations outside allowed paths
6. Simulate a task failure and verify the retry includes previous attempt context

---

## Test File Summary

| File | Type | Phases Covered |
|------|------|----------------|
| `src/context/agents_md.rs` | Unit (inline) | Phase 1 (AC1) |
| `src/context/mod.rs` | Unit (inline) | Phase 1 (AC2), Phase 2 (AC3, AC4) |
| `src/autonomy.rs` | Unit (inline) | Phase 3 (AC5, AC6, AC14) |
| `src/security/scope.rs` | Unit (inline) | Phase 4 (AC7, AC8) |
| `src/tools/search.rs` | Unit (inline) | Phase 5 (AC9, AC10.2) |
| `tests/security_scope_test.rs` | Integration | Phase 4 (AC7, AC8) |
| `tests/code_search_test.rs` | Integration | Phase 5 (AC9) |
| `tests/orchestrator_test.rs` | Integration | Phase 6 (AC11, AC12, AC13) |
