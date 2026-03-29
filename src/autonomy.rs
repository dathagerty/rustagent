use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    Full,
    #[default]
    Supervised,
    Gated,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum ApprovalResponse {
    Approve,
    Reject { reason: String },
    Modify { instructions: String },
}

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
                id: format!(
                    "approval-{}",
                    uuid::Uuid::new_v4()
                        .simple()
                        .to_string()
                        .get(..8)
                        .unwrap_or("00000000")
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Task 1: AutonomyLevel and ApprovalGate types =====

    #[test]
    fn test_full_autonomy_has_no_active_gates() {
        let level = AutonomyLevel::Full;
        assert_eq!(level.active_gates(), vec![]);
    }

    #[test]
    fn test_supervised_autonomy_has_correct_gates() {
        let level = AutonomyLevel::Supervised;
        let gates = level.active_gates();
        assert_eq!(gates.len(), 4);
        assert!(gates.contains(&ApprovalGate::PlanReview));
        assert!(gates.contains(&ApprovalGate::PreCommit));
        assert!(gates.contains(&ApprovalGate::TaskComplete));
        assert!(gates.contains(&ApprovalGate::GoalComplete));
    }

    #[test]
    fn test_gated_autonomy_has_all_gates() {
        let level = AutonomyLevel::Gated;
        let gates = level.active_gates();
        assert_eq!(gates.len(), 6);
        assert!(gates.contains(&ApprovalGate::PlanReview));
        assert!(gates.contains(&ApprovalGate::PreCommit));
        assert!(gates.contains(&ApprovalGate::TaskComplete));
        assert!(gates.contains(&ApprovalGate::DecisionPoint));
        assert!(gates.contains(&ApprovalGate::GoalComplete));
        assert!(gates.contains(&ApprovalGate::WorkerSpawn));
    }

    #[test]
    fn test_default_autonomy_level_is_supervised() {
        assert_eq!(AutonomyLevel::default(), AutonomyLevel::Supervised);
    }

    #[test]
    fn test_is_gate_active() {
        let full = AutonomyLevel::Full;
        assert!(!full.is_gate_active(ApprovalGate::PlanReview));

        let supervised = AutonomyLevel::Supervised;
        assert!(supervised.is_gate_active(ApprovalGate::PlanReview));
        assert!(supervised.is_gate_active(ApprovalGate::PreCommit));
        assert!(supervised.is_gate_active(ApprovalGate::TaskComplete));
        assert!(supervised.is_gate_active(ApprovalGate::GoalComplete));
        assert!(!supervised.is_gate_active(ApprovalGate::DecisionPoint));
        assert!(!supervised.is_gate_active(ApprovalGate::WorkerSpawn));

        let gated = AutonomyLevel::Gated;
        assert!(gated.is_gate_active(ApprovalGate::PlanReview));
        assert!(gated.is_gate_active(ApprovalGate::DecisionPoint));
        assert!(gated.is_gate_active(ApprovalGate::WorkerSpawn));
    }

    // ===== Task 2 & 3: ApprovalRequest, ApprovalResponse, and serde tests =====

    #[test]
    fn test_approval_request_construction() {
        let request = ApprovalRequest {
            id: "approval-12345678".to_string(),
            gate: ApprovalGate::PlanReview,
            context_summary: "Test context".to_string(),
            proposed_action: "Test action".to_string(),
            related_node_id: "ra-1234".to_string(),
        };

        assert_eq!(request.id, "approval-12345678");
        assert_eq!(request.gate, ApprovalGate::PlanReview);
        assert_eq!(request.context_summary, "Test context");
        assert_eq!(request.proposed_action, "Test action");
        assert_eq!(request.related_node_id, "ra-1234");
    }

    #[test]
    fn test_autonomy_level_serde_roundtrip() {
        let levels = vec![
            AutonomyLevel::Full,
            AutonomyLevel::Supervised,
            AutonomyLevel::Gated,
        ];

        for level in levels {
            let json = serde_json::to_string(&level).expect("failed to serialize");
            let deserialized: AutonomyLevel =
                serde_json::from_str(&json).expect("failed to deserialize");
            assert_eq!(level, deserialized);
        }
    }

    #[test]
    fn test_autonomy_level_serde_format() {
        let full_json = serde_json::to_string(&AutonomyLevel::Full).unwrap();
        assert_eq!(full_json, "\"full\"");

        let supervised_json = serde_json::to_string(&AutonomyLevel::Supervised).unwrap();
        assert_eq!(supervised_json, "\"supervised\"");

        let gated_json = serde_json::to_string(&AutonomyLevel::Gated).unwrap();
        assert_eq!(gated_json, "\"gated\"");
    }

    #[test]
    fn test_approval_gate_serde_roundtrip() {
        let gates = vec![
            ApprovalGate::PlanReview,
            ApprovalGate::PreCommit,
            ApprovalGate::TaskComplete,
            ApprovalGate::DecisionPoint,
            ApprovalGate::GoalComplete,
            ApprovalGate::WorkerSpawn,
        ];

        for gate in gates {
            let json = serde_json::to_string(&gate).expect("failed to serialize");
            let deserialized: ApprovalGate =
                serde_json::from_str(&json).expect("failed to deserialize");
            assert_eq!(gate, deserialized);
        }
    }

    #[test]
    fn test_approval_gate_serde_format() {
        let json = serde_json::to_string(&ApprovalGate::PlanReview).unwrap();
        assert_eq!(json, "\"plan_review\"");

        let json = serde_json::to_string(&ApprovalGate::PreCommit).unwrap();
        assert_eq!(json, "\"pre_commit\"");

        let json = serde_json::to_string(&ApprovalGate::TaskComplete).unwrap();
        assert_eq!(json, "\"task_complete\"");

        let json = serde_json::to_string(&ApprovalGate::DecisionPoint).unwrap();
        assert_eq!(json, "\"decision_point\"");

        let json = serde_json::to_string(&ApprovalGate::GoalComplete).unwrap();
        assert_eq!(json, "\"goal_complete\"");

        let json = serde_json::to_string(&ApprovalGate::WorkerSpawn).unwrap();
        assert_eq!(json, "\"worker_spawn\"");
    }

    #[test]
    fn test_approval_response_approve_serde() {
        let response = ApprovalResponse::Approve;
        let json = serde_json::to_string(&response).expect("failed to serialize");
        let deserialized: ApprovalResponse =
            serde_json::from_str(&json).expect("failed to deserialize");
        assert_eq!(json, "{\"action\":\"approve\"}");
        assert_eq!(deserialized, response);
    }

    #[test]
    fn test_approval_response_reject_serde() {
        let response = ApprovalResponse::Reject {
            reason: "Too risky".to_string(),
        };
        let json = serde_json::to_string(&response).expect("failed to serialize");
        let deserialized: ApprovalResponse =
            serde_json::from_str(&json).expect("failed to deserialize");
        assert!(json.contains("\"action\":\"reject\""));
        assert!(json.contains("\"reason\":\"Too risky\""));
        assert_eq!(deserialized, response);
    }

    #[test]
    fn test_approval_response_modify_serde() {
        let response = ApprovalResponse::Modify {
            instructions: "Change the plan".to_string(),
        };
        let json = serde_json::to_string(&response).expect("failed to serialize");
        let deserialized: ApprovalResponse =
            serde_json::from_str(&json).expect("failed to deserialize");
        assert!(json.contains("\"action\":\"modify\""));
        assert!(json.contains("\"instructions\":\"Change the plan\""));
        assert_eq!(deserialized, response);
    }

    #[test]
    fn test_approval_request_serde() {
        let request = ApprovalRequest {
            id: "approval-12345678".to_string(),
            gate: ApprovalGate::PlanReview,
            context_summary: "Planning a new feature".to_string(),
            proposed_action: "Create task nodes".to_string(),
            related_node_id: "ra-abcd".to_string(),
        };

        let json = serde_json::to_string(&request).expect("failed to serialize");
        let deserialized: ApprovalRequest =
            serde_json::from_str(&json).expect("failed to deserialize");

        assert_eq!(deserialized.id, request.id);
        assert_eq!(deserialized.gate, request.gate);
        assert_eq!(deserialized.context_summary, request.context_summary);
        assert_eq!(deserialized.proposed_action, request.proposed_action);
        assert_eq!(deserialized.related_node_id, request.related_node_id);
    }

    // ===== Task 4: GateChecker tests =====

    #[test]
    fn test_gate_checker_full_autonomy_returns_none() {
        let checker = GateChecker::new(AutonomyLevel::Full);
        let result = checker.check_gate(
            ApprovalGate::PlanReview,
            "ra-1234",
            "Test context",
            "Test action",
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_gate_checker_supervised_plan_review_returns_request() {
        let checker = GateChecker::new(AutonomyLevel::Supervised);
        let result = checker.check_gate(
            ApprovalGate::PlanReview,
            "ra-1234",
            "Planning phase",
            "Create subtasks",
        );

        assert!(result.is_some());
        let req = result.unwrap();
        assert_eq!(req.gate, ApprovalGate::PlanReview);
        assert_eq!(req.context_summary, "Planning phase");
        assert_eq!(req.proposed_action, "Create subtasks");
        assert_eq!(req.related_node_id, "ra-1234");
        assert!(req.id.starts_with("approval-"));
    }

    #[test]
    fn test_gate_checker_supervised_inactive_gate_returns_none() {
        let checker = GateChecker::new(AutonomyLevel::Supervised);
        let result = checker.check_gate(
            ApprovalGate::DecisionPoint,
            "ra-1234",
            "Test context",
            "Test action",
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_gate_checker_gated_all_gates_active() {
        let checker = GateChecker::new(AutonomyLevel::Gated);

        let gates = vec![
            ApprovalGate::PlanReview,
            ApprovalGate::PreCommit,
            ApprovalGate::TaskComplete,
            ApprovalGate::DecisionPoint,
            ApprovalGate::GoalComplete,
            ApprovalGate::WorkerSpawn,
        ];

        for gate in gates {
            let result = checker.check_gate(gate, "ra-1234", "Test context", "Test action");
            assert!(
                result.is_some(),
                "Gate {:?} should be active in Gated mode",
                gate
            );
            assert_eq!(result.unwrap().gate, gate);
        }
    }

    #[test]
    fn test_gate_checker_generates_unique_ids() {
        let checker = GateChecker::new(AutonomyLevel::Supervised);

        let req1 = checker
            .check_gate(ApprovalGate::PlanReview, "ra-1234", "Test 1", "Action 1")
            .unwrap();

        let req2 = checker
            .check_gate(ApprovalGate::PlanReview, "ra-5678", "Test 2", "Action 2")
            .unwrap();

        assert_ne!(req1.id, req2.id);
    }
}
