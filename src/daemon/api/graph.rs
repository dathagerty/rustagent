use super::{ApiError, AppState};
use crate::graph::store::{EdgeDirection, NodeQuery};
use crate::graph::{self, GraphEdge, GraphNode, NodeStatus, NodeType, Priority};
use crate::graph::{interchange, session::SessionStore};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

/// Resolve a project path parameter (name or ID) to the actual project ID
pub(super) async fn resolve_project_id(
    state: &AppState,
    id_or_name: &str,
) -> Result<String, ApiError> {
    if let Some(p) = state.project_store.get_by_name(id_or_name).await? {
        return Ok(p.id);
    }
    if let Some(p) = state.project_store.get_by_id(id_or_name).await? {
        return Ok(p.id);
    }
    Err(ApiError::NotFound(format!(
        "Project '{}' not found",
        id_or_name
    )))
}

// ===== Request types =====

#[derive(Deserialize)]
pub struct CreateGoalRequest {
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateNodeRequest {
    pub status: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub blocked_reason: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct CreateChildRequest {
    pub node_type: String,
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct CreateEdgeRequest {
    pub edge_type: String,
    pub from_node: String,
    pub to_node: String,
    pub label: Option<String>,
}

#[derive(Deserialize)]
pub struct ImportRequest {
    pub toml: String,
    pub strategy: Option<String>,
}

// ===== Response types =====

#[derive(Serialize)]
pub struct NodeWithEdges {
    pub node: GraphNode,
    pub incoming_edges: Vec<(GraphEdge, GraphNode)>,
    pub outgoing_edges: Vec<(GraphEdge, GraphNode)>,
}

#[derive(Serialize)]
pub struct GoalTree {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
pub struct DecisionHistory {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
pub struct ExportResult {
    pub goal_id: String,
    pub toml: String,
}

// ===== Goal endpoints =====

/// GET /api/projects/:id/goals
pub async fn list_goals(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let project_id = resolve_project_id(&state, &id_or_name).await?;
    let query = NodeQuery {
        node_type: Some(NodeType::Goal),
        project_id: Some(project_id),
        ..Default::default()
    };
    let goals = state.graph_store.query_nodes(&query).await?;
    Ok(Json(goals))
}

/// POST /api/projects/:id/goals
pub async fn create_goal(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
    Json(body): Json<CreateGoalRequest>,
) -> Result<(StatusCode, Json<GraphNode>), ApiError> {
    let project_id = resolve_project_id(&state, &id_or_name).await?;

    let priority = body
        .priority
        .map(|p| p.parse::<Priority>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let node = GraphNode {
        id: graph::generate_goal_id(),
        project_id,
        node_type: NodeType::Goal,
        title: body.title,
        description: body.description,
        status: NodeStatus::Active,
        priority,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: std::collections::HashMap::new(),
    };

    state.graph_store.create_node(&node).await?;
    Ok((StatusCode::CREATED, Json(node)))
}

// ===== Node endpoints =====

/// GET /api/nodes/:id
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<NodeWithEdges>, ApiError> {
    let node = state
        .graph_store
        .get_node(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Node '{}' not found", id)))?;

    let incoming = state
        .graph_store
        .get_edges(&id, EdgeDirection::Incoming)
        .await?;
    let outgoing = state
        .graph_store
        .get_edges(&id, EdgeDirection::Outgoing)
        .await?;

    Ok(Json(NodeWithEdges {
        node,
        incoming_edges: incoming,
        outgoing_edges: outgoing,
    }))
}

/// PATCH /api/nodes/:id
pub async fn update_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateNodeRequest>,
) -> Result<Json<GraphNode>, ApiError> {
    let existing = state
        .graph_store
        .get_node(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Node '{}' not found", id)))?;

    let status = body
        .status
        .map(|s| s.parse::<NodeStatus>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    if let Some(ref s) = status {
        graph::validate_status(&existing.node_type, s)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    }

    state
        .graph_store
        .update_node(
            &id,
            status,
            body.title.as_deref(),
            body.description.as_deref(),
            body.blocked_reason.as_deref(),
            body.metadata.as_ref(),
        )
        .await?;

    let updated = state
        .graph_store
        .get_node(&id)
        .await?
        .ok_or_else(|| ApiError::Internal("Node disappeared after update".to_string()))?;

    Ok(Json(updated))
}

/// POST /api/nodes/:id/children
pub async fn create_child(
    State(state): State<AppState>,
    Path(parent_id): Path<String>,
    Json(body): Json<CreateChildRequest>,
) -> Result<(StatusCode, Json<GraphNode>), ApiError> {
    let parent = state
        .graph_store
        .get_node(&parent_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Parent node '{}' not found", parent_id)))?;

    let node_type: NodeType = body
        .node_type
        .parse()
        .map_err(|e: anyhow::Error| ApiError::BadRequest(e.to_string()))?;

    let priority = body
        .priority
        .map(|p| p.parse::<Priority>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let seq = state.graph_store.next_child_seq(&parent_id).await?;
    let child_id = graph::generate_child_id(&parent_id, seq);

    let node = GraphNode {
        id: child_id,
        project_id: parent.project_id,
        node_type,
        title: body.title,
        description: body.description,
        status: NodeStatus::Pending,
        priority,
        assigned_to: None,
        created_by: None,
        labels: vec![],
        created_at: chrono::Utc::now(),
        started_at: None,
        completed_at: None,
        blocked_reason: None,
        metadata: body.metadata.unwrap_or_default(),
    };

    // SqliteGraphStore::create_node() auto-creates a Contains edge
    state.graph_store.create_node(&node).await?;
    Ok((StatusCode::CREATED, Json(node)))
}

// ===== Edge endpoints =====

/// POST /api/edges
pub async fn create_edge(
    State(state): State<AppState>,
    Json(body): Json<CreateEdgeRequest>,
) -> Result<(StatusCode, Json<GraphEdge>), ApiError> {
    let edge_type: graph::EdgeType = body
        .edge_type
        .parse()
        .map_err(|e: anyhow::Error| ApiError::BadRequest(e.to_string()))?;

    state
        .graph_store
        .get_node(&body.from_node)
        .await?
        .ok_or_else(|| ApiError::BadRequest(format!("From node '{}' not found", body.from_node)))?;
    state
        .graph_store
        .get_node(&body.to_node)
        .await?
        .ok_or_else(|| ApiError::BadRequest(format!("To node '{}' not found", body.to_node)))?;

    let edge = GraphEdge {
        id: graph::generate_edge_id(),
        edge_type,
        from_node: body.from_node,
        to_node: body.to_node,
        label: body.label,
        created_at: chrono::Utc::now(),
    };

    state.graph_store.add_edge(&edge).await?;
    Ok((StatusCode::CREATED, Json(edge)))
}

/// DELETE /api/edges/:id
pub async fn delete_edge(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.graph_store.remove_edge(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===== Goal tree =====

/// GET /api/goals/:id/tree
pub async fn get_goal_tree(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<GoalTree>, ApiError> {
    state
        .graph_store
        .get_node(&goal_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Goal '{}' not found", goal_id)))?;

    let graph = state.graph_store.get_full_graph(&goal_id).await?;
    Ok(Json(GoalTree {
        nodes: graph.nodes,
        edges: graph.edges,
    }))
}

// ===== Task views =====

/// GET /api/goals/:id/tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let subtree = state.graph_store.get_subtree(&goal_id).await?;
    let tasks: Vec<GraphNode> = subtree
        .into_iter()
        .filter(|n| n.node_type == NodeType::Task)
        .collect();
    Ok(Json(tasks))
}

/// GET /api/goals/:id/tasks/ready
pub async fn list_ready_tasks(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let tasks = state.graph_store.get_ready_tasks(&goal_id).await?;
    Ok(Json(tasks))
}

/// GET /api/goals/:id/tasks/next
pub async fn next_task(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Option<GraphNode>>, ApiError> {
    let task = state.graph_store.get_next_task(&goal_id).await?;
    Ok(Json(task))
}

// ===== Decision views =====

/// GET /api/projects/:id/decisions
pub async fn list_decisions(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let project_id = resolve_project_id(&state, &id_or_name).await?;
    let decisions = state.graph_store.get_active_decisions(&project_id).await?;
    Ok(Json(decisions))
}

/// GET /api/projects/:id/decisions/history
pub async fn decisions_history(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<DecisionHistory>, ApiError> {
    let project_id = resolve_project_id(&state, &id_or_name).await?;
    let decision_types = [
        NodeType::Decision,
        NodeType::Option,
        NodeType::Outcome,
        NodeType::Revisit,
    ];
    let mut all_nodes = Vec::new();

    for node_type in &decision_types {
        let query = NodeQuery {
            node_type: Some(*node_type),
            project_id: Some(project_id.clone()),
            ..Default::default()
        };
        let mut nodes = state.graph_store.query_nodes(&query).await?;
        all_nodes.append(&mut nodes);
    }

    let node_ids: std::collections::HashSet<String> =
        all_nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = Vec::new();
    for node in &all_nodes {
        let outgoing = state
            .graph_store
            .get_edges(&node.id, EdgeDirection::Outgoing)
            .await?;
        for (edge, target) in outgoing {
            if node_ids.contains(&target.id) {
                edges.push(edge);
            }
        }
    }

    Ok(Json(DecisionHistory {
        nodes: all_nodes,
        edges,
    }))
}

/// POST /api/projects/:id/decisions/export
pub async fn export_decisions(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    let project = match state.project_store.get_by_name(&id_or_name).await? {
        Some(p) => Some(p),
        None => state.project_store.get_by_id(&id_or_name).await?,
    }
    .ok_or_else(|| ApiError::NotFound(format!("Project '{}' not found", id_or_name)))?;

    let output_dir = project.path.join("decisions");
    let files =
        crate::graph::export::export_adrs(state.graph_store.as_ref(), &project.id, &output_dir)
            .await?;

    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    Ok(Json(paths))
}

// ===== Session endpoints =====

/// GET /api/goals/:id/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<crate::graph::session::Session>>, ApiError> {
    let session_store = SessionStore::new(state.db.clone());
    let sessions = session_store.list_sessions(&goal_id).await?;
    Ok(Json(sessions))
}

/// GET /api/sessions/:id
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::graph::session::Session>, ApiError> {
    let session_store = SessionStore::new(state.db.clone());
    let session = session_store
        .get_session(&id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Session '{}' not found", id)))?;
    Ok(Json(session))
}

// ===== Graph import/export =====

/// GET /api/projects/:id/graph/export
pub async fn export_all_goals(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<Vec<ExportResult>>, ApiError> {
    let project_id = resolve_project_id(&state, &id_or_name).await?;
    let query = NodeQuery {
        node_type: Some(NodeType::Goal),
        project_id: Some(project_id.clone()),
        ..Default::default()
    };
    let goals = state.graph_store.query_nodes(&query).await?;

    let mut results = Vec::new();
    for goal in goals {
        let toml_content =
            interchange::export_goal(state.graph_store.as_ref(), &goal.id, &project_id).await?;
        results.push(ExportResult {
            goal_id: goal.id,
            toml: toml_content,
        });
    }

    Ok(Json(results))
}

/// GET /api/goals/:id/export
pub async fn export_goal_toml(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<ExportResult>, ApiError> {
    let node = state
        .graph_store
        .get_node(&goal_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Goal '{}' not found", goal_id)))?;

    let toml_content =
        interchange::export_goal(state.graph_store.as_ref(), &goal_id, &node.project_id).await?;

    Ok(Json(ExportResult {
        goal_id,
        toml: toml_content,
    }))
}

/// POST /api/projects/:id/graph/import
pub async fn import_graph(
    State(state): State<AppState>,
    Path(_project_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<interchange::ImportResult>, ApiError> {
    let strategy = match body.strategy.as_deref() {
        Some("theirs") => interchange::ImportStrategy::Theirs,
        Some("ours") => interchange::ImportStrategy::Ours,
        _ => interchange::ImportStrategy::Merge,
    };

    let result = interchange::import_goal(state.graph_store.as_ref(), &body.toml, strategy).await?;

    Ok(Json(result))
}

/// POST /api/projects/:id/graph/diff
pub async fn diff_graph(
    State(state): State<AppState>,
    Path(_project_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<interchange::DiffResult>, ApiError> {
    let result = interchange::diff_goal(state.graph_store.as_ref(), &body.toml).await?;
    Ok(Json(result))
}
