# V2 Phase 5 - Autonomy Levels & Approval Gates

**Goal:** Implement the autonomy level system that controls human oversight granularity, with approval gates that pause operations for user review at configurable points.

**Architecture:** A new `src/autonomy.rs` module defines `AutonomyLevel` (Full/Supervised/Gated), `ApprovalGate` (6 variants), `ApprovalRequest`, and `ApprovalResponse` types. A `GateChecker` determines which gates are active for a given autonomy level. The design places this in `src/config/autonomy.rs` but since `config.rs` is a flat file (not a directory module), we create `src/autonomy.rs` as a top-level module instead — it's a distinct V2 concept, not part of V1 config.

**Tech Stack:** Rust (serde for serialization, tokio oneshot for async approval responses)

**Scope:** 1 of 6 phases from original design (Phase 5, item 3)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC5: Autonomy Level Types
- **v2-phase5.AC5.1 Success:** `AutonomyLevel::Full` has no active gates
- **v2-phase5.AC5.2 Success:** `AutonomyLevel::Supervised` activates PlanReview, PreCommit, TaskComplete, GoalComplete gates
- **v2-phase5.AC5.3 Success:** `AutonomyLevel::Gated` activates all 6 gates (the 4 from Supervised plus DecisionPoint and WorkerSpawn)
- **v2-phase5.AC5.4 Success:** Default autonomy level is `Supervised`

### v2-phase5.AC6: Approval Request/Response
- **v2-phase5.AC6.1 Success:** `ApprovalRequest` contains gate type, context summary, and proposed action
- **v2-phase5.AC6.2 Success:** `ApprovalResponse` supports Approve, Reject(reason), and Modify(instructions)

### v2-phase5.AC14: Gate Checking
- **v2-phase5.AC14.1 Success:** `check_gate` returns `None` when the gate is not active for the current autonomy level
- **v2-phase5.AC14.2 Success:** `check_gate` returns an `ApprovalRequest` when the gate is active, allowing the caller to pause and await a response

---

<!-- START_SUBCOMPONENT_A (tasks 1-4) -->

<!-- START_TASK_1 -->
### Task 1: Create autonomy types module

**Verifies:** v2-phase5.AC5.1, v2-phase5.AC5.2, v2-phase5.AC5.3, v2-phase5.AC5.4

**Files:**
- Create: `src/autonomy.rs`
- Modify: `src/lib.rs` (add `pub mod autonomy;`)

**Implementation:**

Create `src/autonomy.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    Full,
    Supervised,
    Gated,
}

impl Default for AutonomyLevel {
    fn default() -> Self {
        Self::Supervised
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalGate {
    PlanReview,
    PreCommit,
    TaskComplete,
    DecisionPoint,
    GoalComplete,
    WorkerSpawn,
}

impl AutonomyLevel {
    /// Returns the set of gates that are active for this autonomy level.
    pub fn active_gates(&self) -> Vec<ApprovalGate> {
        match self {
            Self::Full => vec![],
            Self::Supervised => vec![
                ApprovalGate::PlanReview,
                ApprovalGate::PreCommit,
                ApprovalGate::TaskComplete,
                ApprovalGate::GoalComplete,
            ],
            Self::Gated => vec![
                ApprovalGate::PlanReview,
                ApprovalGate::PreCommit,
                ApprovalGate::TaskComplete,
                ApprovalGate::DecisionPoint,
                ApprovalGate::GoalComplete,
                ApprovalGate::WorkerSpawn,
            ],
        }
    }

    /// Check whether a specific gate is active for this autonomy level.
    pub fn is_gate_active(&self, gate: ApprovalGate) -> bool {
        self.active_gates().contains(&gate)
    }
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC5.1: `Full.active_gates()` returns empty vec
- v2-phase5.AC5.2: `Supervised.active_gates()` returns exactly PlanReview, PreCommit, TaskComplete, GoalComplete
- v2-phase5.AC5.3: `Gated.active_gates()` returns all 6 gates
- v2-phase5.AC5.4: `AutonomyLevel::default()` is `Supervised`

**Verification:**
Run: `cargo test autonomy`
Expected: All tests pass

**Commit:** `feat(autonomy): add AutonomyLevel and ApprovalGate types`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Add ApprovalRequest and ApprovalResponse types

**Verifies:** v2-phase5.AC6.1, v2-phase5.AC6.2

**Files:**
- Modify: `src/autonomy.rs` (add request/response types)

**Implementation:**

Add to `src/autonomy.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique ID for this request
    pub id: String,
    /// Which gate triggered this request
    pub gate: ApprovalGate,
    /// Human-readable summary of what's being approved
    pub context_summary: String,
    /// Description of the proposed action
    pub proposed_action: String,
    /// ID of the goal/task this relates to
    pub related_node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum ApprovalResponse {
    Approve,
    Reject { reason: String },
    Modify { instructions: String },
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC6.1: ApprovalRequest can be constructed with all required fields and serialized to JSON
- v2-phase5.AC6.2: All three ApprovalResponse variants serialize/deserialize correctly (round-trip)

**Verification:**
Run: `cargo test autonomy`
Expected: All tests pass

**Commit:** `feat(autonomy): add ApprovalRequest and ApprovalResponse types`
<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add serde round-trip tests for autonomy types

**Verifies:** v2-phase5.AC5.1, v2-phase5.AC5.2, v2-phase5.AC5.3, v2-phase5.AC6.1, v2-phase5.AC6.2

**Files:**
- Modify: `src/autonomy.rs` (add `#[cfg(test)]` module with serde tests)

**Implementation:**

Add an inline `#[cfg(test)]` module at the bottom of `src/autonomy.rs` with tests that verify:
1. `AutonomyLevel` serializes to lowercase strings (`"full"`, `"supervised"`, `"gated"`) and deserializes back
2. `ApprovalGate` serializes to snake_case (`"plan_review"`, `"pre_commit"`, etc.) and deserializes back
3. `ApprovalResponse::Approve` serializes as `{"action": "approve"}` and deserializes back
4. `ApprovalResponse::Reject` serializes as `{"action": "reject", "reason": "..."}` and deserializes back
5. `ApprovalResponse::Modify` serializes as `{"action": "modify", "instructions": "..."}` and deserializes back

**Testing:**
These ARE the tests. They verify serde serialization matches the expected wire format that the daemon API and CLI will use.

**Verification:**
Run: `cargo test autonomy`
Expected: All tests pass

**Commit:** `test(autonomy): add serde round-trip tests`
<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Add GateChecker for orchestrator gate integration

**Verifies:** v2-phase5.AC14.1, v2-phase5.AC14.2

**Files:**
- Modify: `src/autonomy.rs` (add `GateChecker` struct)

**Implementation:**

Add a `GateChecker` struct that the orchestrator will use to check gates:

```rust
/// Checks whether a gate requires approval for the current autonomy level.
pub struct GateChecker {
    level: AutonomyLevel,
}

impl GateChecker {
    pub fn new(level: AutonomyLevel) -> Self {
        Self { level }
    }

    /// Check if a gate requires approval. Returns Some(ApprovalRequest) if the gate
    /// is active and approval is needed, None if the gate is inactive.
    pub fn check_gate(
        &self,
        gate: ApprovalGate,
        related_node_id: &str,
        context_summary: &str,
        proposed_action: &str,
    ) -> Option<ApprovalRequest> {
        if self.level.is_gate_active(gate) {
            Some(ApprovalRequest {
                id: format!("approval-{}", uuid::Uuid::new_v4().simple().to_string().get(..8).unwrap_or("00000000")),
                gate,
                context_summary: context_summary.to_string(),
                proposed_action: proposed_action.to_string(),
                related_node_id: related_node_id.to_string(),
            })
        } else {
            None
        }
    }
}
```

Note: The orchestrator integration (calling `check_gate` at the right points in the state machine, pausing operations, and handling responses) is part of Phase 3 (Orchestrator) in the broader V2 architecture, not this phase. This phase provides the types and checker logic so the orchestrator can consume them. The design's orchestrator state machine already has the hookpoints (Planning, Scheduling, Monitoring).

**Testing:**
Tests must verify:
- v2-phase5.AC14.1: `GateChecker::new(Full).check_gate(PlanReview, ...)` returns `None`
- v2-phase5.AC14.2: `GateChecker::new(Supervised).check_gate(PlanReview, ...)` returns `Some(ApprovalRequest)` with correct gate and fields

**Verification:**
Run: `cargo test autonomy`
Expected: All tests pass

**Commit:** `feat(autonomy): add GateChecker for orchestrator integration`
<!-- END_TASK_4 -->

<!-- END_SUBCOMPONENT_A -->
