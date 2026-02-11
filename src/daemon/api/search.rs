use super::{ApiError, AppState};
use crate::graph::{GraphNode, NodeType};
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub node_type: Option<String>,
    pub limit: Option<usize>,
}

/// POST /api/projects/:id/search
pub async fn search_nodes(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let project_id = super::graph::resolve_project_id(&state, &id_or_name).await?;

    let node_type = body
        .node_type
        .map(|t| t.parse::<NodeType>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let limit = body.limit.unwrap_or(50);

    let results = state
        .graph_store
        .search_nodes(&body.query, Some(&project_id), node_type, limit)
        .await?;

    Ok(Json(results))
}
