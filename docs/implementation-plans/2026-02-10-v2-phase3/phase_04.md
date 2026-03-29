# Rustagent V2 Phase 3d: Task/Decision Views + Search + Sessions + Agents + Graph Import/Export API

**Goal:** Build the remaining REST API endpoints — task views (list/ready/next), decision views (now/history/export), full-text search, sessions, agent status, and graph TOML import/export.

**Architecture:** These endpoints are projections of the unified work graph. Task views filter to Task nodes with specific statuses. Decision views filter to Decision/Option/Outcome nodes. Search uses the existing FTS5 index. Sessions are temporal records. Agent status shows active workers. Graph import/export uses the existing TOML interchange module.

**Tech Stack:** Rust (edition 2024), axum 0.8, serde_json, tokio 1.43

**Scope:** Phase 4 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3d.AC1: Task view endpoints
- **P3d.AC1.1 Success:** `GET /api/goals/:id/tasks` returns all task nodes under the goal
- **P3d.AC1.2 Success:** `GET /api/goals/:id/tasks/ready` returns only Ready task nodes
- **P3d.AC1.3 Success:** `GET /api/goals/:id/tasks/next` returns the highest-priority Ready task or `null` if none

### P3d.AC2: Decision view endpoints
- **P3d.AC2.1 Success:** `GET /api/projects/:id/decisions` returns active decision nodes (Now mode)
- **P3d.AC2.2 Success:** `GET /api/projects/:id/decisions/history` returns `DecisionHistory` with `nodes` (Decision, Option, Outcome, Revisit types, all statuses) and `edges` (LeadsTo, Chosen, Rejected, Supersedes) arrays
- **P3d.AC2.3 Success:** `POST /api/projects/:id/decisions/export` triggers ADR markdown export and returns the list of generated file paths

### P3d.AC3: Search endpoint
- **P3d.AC3.1 Success:** `POST /api/projects/:id/search` with `{"query": "auth"}` returns matching nodes via FTS5
- **P3d.AC3.2 Success:** Search supports optional `node_type` filter
- **P3d.AC3.3 Success:** Search supports optional `limit` parameter (default 50)

### P3d.AC4: Session endpoints
- **P3d.AC4.1 Success:** `GET /api/goals/:id/sessions` returns sessions for the goal
- **P3d.AC4.2 Success:** `GET /api/sessions/:id` returns a single session with handoff notes

### P3d.AC5: Agent status endpoint
- **P3d.AC5.1 Success:** `GET /api/goals/:id/agents` returns active agents (tasks with InProgress status and assigned_to set)

### P3d.AC6: Graph import/export endpoints
- **P3d.AC6.1 Success:** `GET /api/projects/:id/graph/export` returns all goals as TOML strings (one per goal)
- **P3d.AC6.2 Success:** `GET /api/goals/:id/export` returns a single goal's TOML representation
- **P3d.AC6.3 Success:** `POST /api/projects/:id/graph/import` accepts TOML body and imports it, returns import result
- **P3d.AC6.4 Success:** `POST /api/projects/:id/graph/diff` accepts TOML body and returns diff against DB state

---

### Prerequisite Changes

Before implementing Phase 3d tasks, apply these changes to existing code:

1. **Add `Serialize` to `ImportResult`, `DiffResult`, and `ImportConflict`** in `src/graph/interchange.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize)]
   pub struct ImportConflict { ... }

   #[derive(Debug, Clone, Serialize)]
   pub struct ImportResult { ... }

   #[derive(Debug, Clone, Serialize)]
   pub struct DiffResult { ... }
   ```
   These types are returned as JSON API responses in Phase 3d Task 6.

2. **Refactor interchange and export functions to accept `&dyn GraphStore`** instead of `&SqliteGraphStore`:

   In `src/graph/interchange.rs`, change:
   ```rust
   // Before:
   pub async fn export_goal(graph_store: &SqliteGraphStore, ...) -> Result<String>
   pub async fn import_goal(graph_store: &SqliteGraphStore, ...) -> Result<ImportResult>
   pub async fn diff_goal(graph_store: &SqliteGraphStore, ...) -> Result<DiffResult>

   // After:
   pub async fn export_goal(graph_store: &dyn GraphStore, ...) -> Result<String>
   pub async fn import_goal(graph_store: &dyn GraphStore, ...) -> Result<ImportResult>
   pub async fn diff_goal(graph_store: &dyn GraphStore, ...) -> Result<DiffResult>
   ```

   In `src/graph/export.rs`, change:
   ```rust
   // Before:
   pub async fn export_adrs(graph_store: &SqliteGraphStore, ...) -> Result<Vec<PathBuf>>

   // After:
   pub async fn export_adrs(graph_store: &dyn GraphStore, ...) -> Result<Vec<PathBuf>>
   ```

   These functions currently only use `GraphStore` trait methods (`query_nodes`, `get_edges`, `get_subtree`, `get_full_graph`), so switching to `&dyn GraphStore` requires no logic changes — only the parameter type. This is necessary because `AppState` holds `Arc<dyn GraphStore>`, and `.as_ref()` gives `&dyn GraphStore`, not `&SqliteGraphStore`.

   Note: `import_goal` uses `SqliteGraphStore::import_nodes_and_edges` — this helper must be added to the `GraphStore` trait or its logic inlined using existing trait methods (`create_node`, `add_edge`).

---

<!-- START_SUBCOMPONENT_A (tasks 1-6) -->

<!-- START_TASK_1 -->
### Task 1: Add task view endpoints

**Verifies:** P3d.AC1.1, P3d.AC1.2, P3d.AC1.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add task view handlers
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount task routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add task view tests

**Implementation:**

Add to `graph.rs`:

```rust
/// GET /api/goals/:id/tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    // Get all nodes under the goal and filter to tasks
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
```

**Route mounting:**

```rust
.route("/api/goals/{id}/tasks", get(graph::list_tasks))
.route("/api/goals/{id}/tasks/ready", get(graph::list_ready_tasks))
.route("/api/goals/{id}/tasks/next", get(graph::next_task))
```

**Testing:**

- P3d.AC1.1: Create goal with 2 task children. GET tasks returns 2 items, both have `node_type: task`.
- P3d.AC1.2: Create goal, add 1 Ready task and 1 Pending task. GET ready returns only the Ready task.
- P3d.AC1.3: Create goal with Ready tasks. GET next returns one task. With no Ready tasks, returns `null`.

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): task view endpoints (list, ready, next)`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Add decision view endpoints

**Verifies:** P3d.AC2.1, P3d.AC2.2, P3d.AC2.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add decision view handlers
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount decision routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add decision view tests

**Implementation:**

```rust
/// GET /api/projects/:id/decisions
pub async fn list_decisions(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let decisions = state.graph_store.get_active_decisions(&project_id).await?;
    Ok(Json(decisions))
}

/// GET /api/projects/:id/decisions/history
/// Returns the full decision graph: Decision, Option, Outcome, and Revisit nodes.
/// This matches the architecture's "History mode" which shows the full evolution
/// including Abandoned, Superseded, and Rejected paths.
pub async fn decisions_history(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<DecisionHistory>, ApiError> {
    // Query all decision-related node types for this project
    let decision_types = [NodeType::Decision, NodeType::Option, NodeType::Outcome, NodeType::Revisit];
    let mut all_nodes = Vec::new();

    for node_type in &decision_types {
        let query = NodeQuery {
            node_type: Some(node_type.clone()),
            project_id: Some(project_id.clone()),
            ..Default::default()
        };
        let mut nodes = state.graph_store.query_nodes(&query).await?;
        all_nodes.append(&mut nodes);
    }

    // Collect edges between these nodes (LeadsTo, Chosen, Rejected, Supersedes)
    let node_ids: std::collections::HashSet<String> = all_nodes.iter().map(|n| n.id.clone()).collect();
    let mut edges = Vec::new();
    for node in &all_nodes {
        let outgoing = state.graph_store.get_edges(&node.id, EdgeDirection::Outgoing).await?;
        for (edge, target) in outgoing {
            if node_ids.contains(&target.id) {
                edges.push(edge);
            }
        }
    }

    Ok(Json(DecisionHistory { nodes: all_nodes, edges }))
}
```

Add the response type:

```rust
#[derive(Serialize)]
pub struct DecisionHistory {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// POST /api/projects/:id/decisions/export
pub async fn export_decisions(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    // Look up project by name first, then by ID
    let project = match state.project_store.get_by_name(&project_id).await? {
        Some(p) => Some(p),
        None => state.project_store.get_by_id(&project_id).await?,
    }
    .ok_or_else(|| ApiError::NotFound(format!("Project '{}' not found", project_id)))?;

    let output_dir = project.path.join("decisions");
    let files = crate::graph::export::export_adrs(
        state.graph_store.as_ref(),
        &project.id,
        &output_dir,
    ).await?;

    let paths: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
    Ok(Json(paths))
}
```

**Route mounting:**

```rust
.route("/api/projects/{id}/decisions", get(graph::list_decisions))
.route("/api/projects/{id}/decisions/history", get(graph::decisions_history))
.route("/api/projects/{id}/decisions/export", post(graph::export_decisions))
```

**Testing:**

- P3d.AC2.1: Create project + goal + active decision. GET decisions returns the active one.
- P3d.AC2.2: Create a decided decision with chosen/rejected options and an outcome. GET history returns `DecisionHistory` with all Decision, Option, Outcome nodes and their edges.
- P3d.AC2.3: ADR export test uses tempdir as project path. POST export returns file paths.

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): decision view endpoints (now, history, export)`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add search endpoint

**Verifies:** P3d.AC3.1, P3d.AC3.2, P3d.AC3.3

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/search.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs` — add `pub mod search;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount search route
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_search_api_test.rs`

**Implementation:**

`src/daemon/api/search.rs`:

```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use crate::graph::{GraphNode, NodeType};
use super::{AppState, ApiError};

#[derive(Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub node_type: Option<String>,
    pub limit: Option<usize>,
}

/// POST /api/projects/:id/search
pub async fn search_nodes(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(body): Json<SearchRequest>,
) -> Result<Json<Vec<GraphNode>>, ApiError> {
    let node_type = body.node_type
        .map(|t| t.parse::<NodeType>())
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let limit = body.limit.unwrap_or(50);

    let results = state.graph_store.search_nodes(
        &body.query,
        Some(&project_id),
        node_type,
        limit,
    ).await?;

    Ok(Json(results))
}
```

**Route mounting:**

```rust
.route("/api/projects/{id}/search", post(search::search_nodes))
```

**Testing:**

- P3d.AC3.1: Create nodes with "authentication" in title. Search for "auth" returns matches.
- P3d.AC3.2: Create task and decision nodes. Search with `node_type: "task"` returns only tasks.
- P3d.AC3.3: Create 10 nodes. Search with `limit: 3` returns at most 3 results.

**Verification:**

Run: `cargo test daemon_search_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): full-text search endpoint`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Add session endpoints

**Verifies:** P3d.AC4.1, P3d.AC4.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add session handlers
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount session routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add session tests

**Implementation:**

```rust
use crate::graph::session::{Session, SessionStore};

/// GET /api/goals/:id/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<Vec<Session>>, ApiError> {
    let session_store = SessionStore::new(state.db.clone());
    let sessions = session_store.list_sessions(&goal_id).await?;
    Ok(Json(sessions))
}

/// GET /api/sessions/:id
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Session>, ApiError> {
    let session_store = SessionStore::new(state.db.clone());
    let session = session_store.get_session(&id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Session '{}' not found", id)))?;
    Ok(Json(session))
}
```

**Route mounting:**

```rust
.route("/api/goals/{id}/sessions", get(graph::list_sessions))
.route("/api/sessions/{id}", get(graph::get_session))
```

Note: `SessionStore::get_session(&self, id: &str)` may need to be added if it doesn't exist. Currently `SessionStore` has `get_latest_session` and `list_sessions`. A `get_session` method by ID would query: `SELECT * FROM sessions WHERE id = ?1`.

**Testing:**

- P3d.AC4.1: Create session via SessionStore, GET sessions by goal returns it.
- P3d.AC4.2: GET session by ID returns the full session including handoff_notes.

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): session list and detail endpoints`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Add agent status endpoint

**Verifies:** P3d.AC5.1

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/agents.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs` — add `pub mod agents;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount agents route
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add agents test

**Implementation:**

`src/daemon/api/agents.rs`:

```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use crate::graph::{GraphNode, NodeType, NodeStatus};
use crate::graph::store::NodeQuery;
use super::{AppState, ApiError};

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
    // Get all InProgress tasks under the goal with assigned_to set
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
```

**Route mounting:**

```rust
.route("/api/goals/{id}/agents", get(agents::list_agents))
```

**Testing:**

- P3d.AC5.1: Create goal + task with status InProgress and assigned_to "worker-1". GET agents returns the agent entry. With no InProgress tasks, returns empty array.

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): active agent status endpoint`

<!-- END_TASK_5 -->

<!-- START_TASK_6 -->
### Task 6: Add graph import/export endpoints

**Verifies:** P3d.AC6.1, P3d.AC6.2, P3d.AC6.3, P3d.AC6.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/graph.rs` — add import/export handlers
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount import/export routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_graph_api_test.rs` — add import/export tests

**Implementation:**

```rust
use crate::graph::interchange;

#[derive(Serialize)]
pub struct ExportResult {
    pub goal_id: String,
    pub toml: String,
}

#[derive(Deserialize)]
pub struct ImportRequest {
    pub toml: String,
    pub strategy: Option<String>,  // "merge" (default), "theirs", "ours"
}

/// GET /api/projects/:id/graph/export
pub async fn export_all_goals(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<ExportResult>>, ApiError> {
    let query = NodeQuery {
        node_type: Some(NodeType::Goal),
        project_id: Some(project_id.clone()),
        ..Default::default()
    };
    let goals = state.graph_store.query_nodes(&query).await?;

    let mut results = Vec::new();
    for goal in goals {
        let toml_content = interchange::export_goal(
            state.graph_store.as_ref(),
            &goal.id,
            &project_id,
        ).await?;
        results.push(ExportResult {
            goal_id: goal.id,
            toml: toml_content,
        });
    }

    Ok(Json(results))
}

/// GET /api/goals/:id/export
pub async fn export_goal(
    State(state): State<AppState>,
    Path(goal_id): Path<String>,
) -> Result<Json<ExportResult>, ApiError> {
    let node = state.graph_store.get_node(&goal_id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Goal '{}' not found", goal_id)))?;

    let toml_content = interchange::export_goal(
        state.graph_store.as_ref(),
        &goal_id,
        &node.project_id,
    ).await?;

    Ok(Json(ExportResult { goal_id, toml: toml_content }))
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

    let result = interchange::import_goal(
        state.graph_store.as_ref(),
        &body.toml,
        strategy,
    ).await?;

    Ok(Json(result))
}

/// POST /api/projects/:id/graph/diff
pub async fn diff_graph(
    State(state): State<AppState>,
    Path(_project_id): Path<String>,
    Json(body): Json<ImportRequest>,
) -> Result<Json<interchange::DiffResult>, ApiError> {
    let result = interchange::diff_goal(
        state.graph_store.as_ref(),
        &body.toml,
    ).await?;

    Ok(Json(result))
}
```

Note: `interchange::ImportResult` and `interchange::DiffResult` need to derive `Serialize` if they don't already. Check `src/graph/interchange.rs` and add `#[derive(Serialize)]` to these types.

**Route mounting:**

```rust
.route("/api/projects/{id}/graph/export", get(graph::export_all_goals))
.route("/api/goals/{id}/export", get(graph::export_goal))
.route("/api/projects/{id}/graph/import", post(graph::import_graph))
.route("/api/projects/{id}/graph/diff", post(graph::diff_graph))
```

**Testing:**

- P3d.AC6.1: Create project + goal + tasks. Export all goals returns TOML with nodes.
- P3d.AC6.2: Export single goal returns valid TOML.
- P3d.AC6.3: Export goal, import the exported TOML (round-trip).
- P3d.AC6.4: Export goal, modify it, diff against DB shows changes.

**Verification:**

Run: `cargo test daemon_graph_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): graph TOML import/export/diff endpoints`

<!-- END_TASK_6 -->
<!-- END_SUBCOMPONENT_A -->
