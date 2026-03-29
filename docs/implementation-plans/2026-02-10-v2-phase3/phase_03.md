# Rustagent V2 Phase 3c: Graph Node/Edge API

**Goal:** Build the REST API endpoints for the unified work graph — CRUD operations on nodes and edges, goal management, and subtree queries.

**Architecture:** The graph API exposes the same operations available via the `GraphStore` trait as HTTP endpoints. Graph types (`GraphNode`, `GraphEdge`) already implement `Serialize`/`Deserialize` so JSON responses reuse them directly. Request bodies use dedicated types to validate input.

**Tech Stack:** Rust (edition 2024), axum 0.8, serde_json, tokio 1.43

**Scope:** Phase 3 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3c.AC1: Goal endpoints
- **P3c.AC1.1 Success:** `GET /api/projects/:id/goals` returns JSON array of goal nodes for the project
- **P3c.AC1.2 Success:** `POST /api/projects/:id/goals` with `{"title": "...", "description": "..."}` creates a goal node with auto-generated ID and returns it with 201
- **P3c.AC1.3 Success:** Created goal has `node_type: goal`, `status: active`, `project_id` matching the URL parameter

### P3c.AC2: Node endpoints
- **P3c.AC2.1 Success:** `GET /api/nodes/:id` returns the node with its incoming and outgoing edges
- **P3c.AC2.2 Success:** `GET /api/nodes/:id` returns 404 for nonexistent node
- **P3c.AC2.3 Success:** `PATCH /api/nodes/:id` with `{"status": "completed"}` updates the node status
- **P3c.AC2.4 Success:** `PATCH /api/nodes/:id` with `{"title": "new title", "description": "new desc"}` updates those fields
- **P3c.AC2.5 Success:** `PATCH /api/nodes/:id` with invalid status for the node type returns 400
- **P3c.AC2.6 Success:** `POST /api/nodes/:id/children` with `{"node_type": "task", "title": "...", "description": "..."}` creates a child node under the parent

### P3c.AC3: Edge endpoints
- **P3c.AC3.1 Success:** `POST /api/edges` with `{"edge_type": "depends_on", "from_node": "...", "to_node": "..."}` creates an edge and returns it with 201
- **P3c.AC3.2 Success:** `DELETE /api/edges/:id` removes the edge and returns 204
- **P3c.AC3.3 Success:** Creating an edge with nonexistent node IDs returns 400

### P3c.AC4: Goal tree endpoint
- **P3c.AC4.1 Success:** `GET /api/goals/:id/tree` returns the full subtree (all descendant nodes and edges) under a goal
- **P3c.AC4.2 Success:** Response contains both `nodes` and `edges` arrays
- **P3c.AC4.3 Success:** Returns 404 if goal ID doesn't exist

---

### Prerequisite Changes

Before implementing Phase 3c tasks, apply these changes to existing code:

1. **Add `Default` derive to `NodeQuery`** in `src/graph/store.rs`:
   ```rust
   #[derive(Debug, Clone, Default)]
   pub struct NodeQuery { ... }
   ```
   All fields are `Option<T>`, so `Default` produces a query with no filters (matches all nodes).

2. **Document goal creation behavior**: `POST /api/projects/:id/goals` creates a goal node but does NOT automatically start orchestration. Orchestration is triggered via the `run` CLI command or future daemon-managed workflows. This is a known deviation from the architecture doc's parenthetical note "(starts orchestration)".

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create graph API module with goal and node endpoints

**Verifies:** P3c.AC1.1, P3c.AC1.2, P3c.AC1.3, P3c.AC2.1, P3c.AC2.2, P3c.AC2.3, P3c.AC2.4, P3c.AC2.5, P3c.AC2.6

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs` — add `pub mod graph;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount graph routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs`

**Implementation:**

`src/daemon/api/graph.rs`:

**Request types:**

```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateGoalRequest {
    pub title: String,
    pub description: String,
    pub priority: Option<String>,  // "critical", "high", "medium", "low"
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
```

**Response types:**

```rust
use serde::Serialize;
use crate::graph::{GraphNode, GraphEdge};

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
```

**Handlers:**

```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use super::{AppState, ApiError};
use crate::graph::{self, NodeType, NodeStatus, Priority};
use crate::graph::store::{NodeQuery, EdgeDirection};

/// GET /api/projects/:id/goals
pub async fn list_goals(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
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
    Path(project_id): Path<String>,
    Json(body): Json<CreateGoalRequest>,
) -> Result<(StatusCode, Json<GraphNode>), ApiError> {
    let priority = body.priority
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

/// GET /api/nodes/:id
pub async fn get_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<NodeWithEdges>, ApiError> {
    let node = state.graph_store.get_node(&id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Node '{}' not found", id)))?;

    let incoming = state.graph_store.get_edges(&id, EdgeDirection::Incoming).await?;
    let outgoing = state.graph_store.get_edges(&id, EdgeDirection::Outgoing).await?;

    Ok(Json(NodeWithEdges { node, incoming_edges: incoming, outgoing_edges: outgoing }))
}

/// PATCH /api/nodes/:id
pub async fn update_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateNodeRequest>,
) -> Result<Json<GraphNode>, ApiError> {
    // Verify node exists
    let existing = state.graph_store.get_node(&id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Node '{}' not found", id)))?;

    // Parse and validate status if provided
    let status = body.status
        .map(|s| s.parse::<NodeStatus>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    if let Some(ref s) = status {
        graph::validate_status(&existing.node_type, s)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    }

    state.graph_store.update_node(
        &id,
        status,
        body.title.as_deref(),
        body.description.as_deref(),
        body.blocked_reason.as_deref(),
        body.metadata.as_ref(),
    ).await?;

    let updated = state.graph_store.get_node(&id).await?
        .ok_or_else(|| ApiError::Internal("Node disappeared after update".to_string()))?;

    Ok(Json(updated))
}

/// POST /api/nodes/:id/children
pub async fn create_child(
    State(state): State<AppState>,
    Path(parent_id): Path<String>,
    Json(body): Json<CreateChildRequest>,
) -> Result<(StatusCode, Json<GraphNode>), ApiError> {
    // Verify parent exists
    let parent = state.graph_store.get_node(&parent_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Parent node '{}' not found", parent_id)))?;

    let node_type: NodeType = body.node_type.parse()
        .map_err(|e: anyhow::Error| ApiError::BadRequest(e.to_string()))?;

    let priority = body.priority
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

    // Note: SqliteGraphStore::create_node() auto-creates a Contains edge from
    // parent to child when the node ID is hierarchical (has a dot-separated parent).
    // No explicit add_edge call is needed here.
    state.graph_store.create_node(&node).await?;
    Ok((StatusCode::CREATED, Json(node)))
}
```

**Route mounting** in `server.rs`:

```rust
use crate::daemon::api::graph;

// Add to create_router():
.route("/api/projects/{id}/goals", get(graph::list_goals).post(graph::create_goal))
.route("/api/nodes/{id}", get(graph::get_node).patch(graph::update_node))
.route("/api/nodes/{id}/children", post(graph::create_child))
```

**Testing:**

Tests in `tests/daemon_graph_api_test.rs`:

- P3c.AC1.1: Create a project, create a goal via POST, then GET goals returns 1 item
- P3c.AC1.2: POST goal returns 201 with valid GraphNode JSON
- P3c.AC1.3: Created goal has correct node_type, status, project_id
- P3c.AC2.1: Create a goal, GET node by ID returns node with edges
- P3c.AC2.2: GET nonexistent node returns 404
- P3c.AC2.3: PATCH node with status "completed" updates it
- P3c.AC2.4: PATCH node with title/description updates those fields
- P3c.AC2.5: PATCH goal node with status "ready" (invalid for Goal) returns 400
- P3c.AC2.6: POST child under goal creates a child with correct parent_id hierarchy in ID

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): graph goal and node REST API endpoints`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Add edge endpoints

**Verifies:** P3c.AC3.1, P3c.AC3.2, P3c.AC3.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add edge handlers
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount edge routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add edge tests

**Implementation:**

Add to `graph.rs`:

**Request type:**

```rust
#[derive(Deserialize)]
pub struct CreateEdgeRequest {
    pub edge_type: String,
    pub from_node: String,
    pub to_node: String,
    pub label: Option<String>,
}
```

**Handlers:**

```rust
/// POST /api/edges
pub async fn create_edge(
    State(state): State<AppState>,
    Json(body): Json<CreateEdgeRequest>,
) -> Result<(StatusCode, Json<GraphEdge>), ApiError> {
    let edge_type: graph::EdgeType = body.edge_type.parse()
        .map_err(|e: anyhow::Error| ApiError::BadRequest(e.to_string()))?;

    // Verify both nodes exist
    state.graph_store.get_node(&body.from_node).await?
        .ok_or_else(|| ApiError::BadRequest(format!("From node '{}' not found", body.from_node)))?;
    state.graph_store.get_node(&body.to_node).await?
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
```

**Route mounting:**

```rust
.route("/api/edges", post(graph::create_edge))
.route("/api/edges/{id}", delete(graph::delete_edge))
```

**Testing:**

- P3c.AC3.1: Create two nodes, POST edge between them returns 201
- P3c.AC3.2: Create edge, DELETE it returns 204
- P3c.AC3.3: POST edge with nonexistent from_node returns 400

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): edge create/delete REST API endpoints`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add goal tree endpoint

**Verifies:** P3c.AC4.1, P3c.AC4.2, P3c.AC4.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add tree handler
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount tree route
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add tree tests

**Implementation:**

```rust
/// GET /api/goals/:id/tree
pub async fn get_goal_tree(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<GoalTree>, ApiError> {
    // Verify goal exists
    state.graph_store.get_node(&goal_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Goal '{}' not found", goal_id)))?;

    let graph = state.graph_store.get_full_graph(&goal_id).await?;
    Ok(Json(GoalTree {
        nodes: graph.nodes,
        edges: graph.edges,
    }))
}
```

**Route mounting:**

```rust
.route("/api/goals/{id}/tree", get(graph::get_goal_tree))
```

**Testing:**

- P3c.AC4.1: Create goal with children, GET tree returns all descendants
- P3c.AC4.2: Response has both `nodes` and `edges` arrays
- P3c.AC4.3: GET tree for nonexistent goal returns 404

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): goal tree endpoint for subtree queries`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
