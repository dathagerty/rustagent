use super::{ApiError, AppState};
use crate::graph::{NodeStatus, NodeType};
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct ActiveAgent {
    pub agent_id: String,
    pub task_id: String,
    pub task_title: String,
    pub task_status: NodeStatus,
}

/// GET /api/goals/:id/agents
pub async fn list_agents(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<ActiveAgent>>, ApiError> {
    let subtree = state.graph_store.get_subtree(&goal_id).await?;
    let agents: Vec<ActiveAgent> = subtree
        .into_iter()
        .filter(|n| {
            n.node_type == NodeType::Task
                && n.status == NodeStatus::InProgress
                && n.assigned_to.is_some()
        })
        .map(|n| ActiveAgent {
            agent_id: n.assigned_to.clone().unwrap_or_default(),
            task_id: n.id.clone(),
            task_title: n.title.clone(),
            task_status: n.status,
        })
        .collect();

    Ok(Json(agents))
}
