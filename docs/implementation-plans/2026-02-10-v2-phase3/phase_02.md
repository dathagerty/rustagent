# Rustagent V2 Phase 3b: HTTP Server Foundation + Project API

**Goal:** Build the axum HTTP server with shared application state, CORS middleware, a health endpoint, and the Project CRUD REST API as the first set of endpoints.

**Architecture:** The daemon's axum server serves `/api/*` for REST, `/ws` for WebSocket (Phase 3e), and `/*` for UI assets (Phase 3g). Shared state (`AppState`) holds the Database, GraphStore, ProjectStore, MessageBus, active orchestrators, and a WebSocket event broadcaster. All API responses use a consistent JSON envelope.

**Tech Stack:** Rust (edition 2024), axum 0.8, tower-http 0.6 (CORS), serde_json, tokio 1.43

**Scope:** Phase 2 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3b.AC1: AppState
- **P3b.AC1.1 Success:** `AppState` contains `db: Database`, `graph_store: Arc<dyn GraphStore>`, `project_store: ProjectStore`, `message_bus: Arc<dyn MessageBus>`, `ws_tx: broadcast::Sender<WsEvent>`, `orchestrators: Arc<Mutex<HashMap<String, OrchestratorHandle>>>`
- **P3b.AC1.2 Success:** `AppState` implements Clone (all fields are Arc-wrapped or Clone)

### P3b.AC2: Server startup
- **P3b.AC2.1 Success:** `create_router(state: AppState) -> Router` returns a configured axum Router
- **P3b.AC2.2 Success:** CORS middleware allows all origins in development (configurable)
- **P3b.AC2.3 Success:** `GET /api/health` returns `200 OK` with `{"status": "ok"}`

### P3b.AC3: API error handling
- **P3b.AC3.1 Success:** `ApiError` type implements `IntoResponse` and returns structured JSON errors with HTTP status codes
- **P3b.AC3.2 Success:** 404 errors return `{"error": "not found", "message": "..."}`
- **P3b.AC3.3 Success:** 400 errors return `{"error": "bad request", "message": "..."}`
- **P3b.AC3.4 Success:** 500 errors return `{"error": "internal error", "message": "..."}`

### P3b.AC4: Project API
- **P3b.AC4.1 Success:** `GET /api/projects` returns JSON array of all projects
- **P3b.AC4.2 Success:** `POST /api/projects` with `{"name": "...", "path": "..."}` creates a project and returns it
- **P3b.AC4.3 Success:** `GET /api/projects/:id` returns project details or 404
- **P3b.AC4.4 Success:** `DELETE /api/projects/:id` removes a project and returns 204
- **P3b.AC4.5 Success:** `POST /api/projects` with duplicate name returns 409 Conflict

### P3b.AC5: Server integration with daemon
- **P3b.AC5.1 Success:** `start_server(config: &DaemonConfig, state: AppState, shutdown: CancellationToken)` starts axum on the configured address and shuts down when the token is cancelled
- **P3b.AC5.2 Success:** Daemon `start` command creates AppState and passes it to `start_server`

---

<!-- START_SUBCOMPONENT_A (tasks 1-4) -->

<!-- START_TASK_1 -->
### Task 1: Create AppState and API error types

**Verifies:** P3b.AC1.1, P3b.AC1.2, P3b.AC3.1, P3b.AC3.2, P3b.AC3.3, P3b.AC3.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_api_test.rs`

**Implementation:**

Create the `src/daemon/api/` directory with `mod.rs` as the module root.

**1. WsEvent placeholder** (full implementation in Phase 3e):

```rust
/// WebSocket event type (fully defined in Phase 3e)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    Heartbeat { timestamp: String },
}
```

**2. OrchestratorHandle** (lightweight handle for managing running orchestrators):

Note: `OrchestratorHandle` is defined in `api/mod.rs` for now since it's used by `AppState`. If the daemon module grows, consider moving it to `src/daemon/mod.rs` and re-exporting — it's an orchestration concept, not an API concept.

```rust
pub struct OrchestratorHandle {
    pub goal_id: String,
    pub project_id: String,
    pub cancel_token: tokio_util::sync::CancellationToken,
    pub started_at: chrono::DateTime<chrono::Utc>,
}
```

**3. AppState:**

```rust
use crate::db::Database;
use crate::graph::store::{GraphStore, SqliteGraphStore};
use crate::message::MessageBus;
use crate::project::ProjectStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub graph_store: Arc<dyn GraphStore>,
    pub project_store: ProjectStore,
    pub message_bus: Arc<dyn MessageBus>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub orchestrators: Arc<Mutex<HashMap<String, OrchestratorHandle>>>,
}

impl AppState {
    pub fn new(
        db: Database,
        graph_store: Arc<dyn GraphStore>,
        message_bus: Arc<dyn MessageBus>,
    ) -> Self {
        let (ws_tx, _) = broadcast::channel(256);
        Self {
            project_store: ProjectStore::new(db.clone()),
            db,
            graph_store,
            message_bus,
            ws_tx,
            orchestrators: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
```

Note: `ProjectStore` needs to implement `Clone`. Currently it wraps `Database` which is Clone. Add `#[derive(Clone)]` to `ProjectStore` in `src/project.rs` if not already present.

**4. ApiError:**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad request", msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            ApiError::Internal(msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error", msg)
            }
        };

        let body = serde_json::json!({
            "error": error_type,
            "message": message,
        });

        (status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}
```

**Testing:**

Tests in `tests/daemon_api_test.rs`:

- P3b.AC1.2: Construct AppState with in-memory DB. Clone it. Both copies share the same Arc-wrapped resources.
- P3b.AC3.1-4: Construct each ApiError variant, call `into_response()`, verify the status code and JSON body structure.

**Verification:**

Run: `cargo test daemon_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): AppState, ApiError, and WsEvent foundation types`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create axum server with health endpoint and CORS

**Verifies:** P3b.AC2.1, P3b.AC2.2, P3b.AC2.3, P3b.AC5.1

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_server_test.rs`

**Implementation:**

```rust
use crate::daemon::api::AppState;
use crate::daemon::DaemonConfig;
use axum::{Json, Router, routing::get};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

/// Create the axum Router with all routes and middleware
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_check))
        // Project routes (Phase 3b, Task 3)
        // Graph routes (Phase 3c)
        // WebSocket route (Phase 3e)
        .layer(cors)
        .with_state(state)
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Start the axum server, blocking until the shutdown token is cancelled
pub async fn start_server(
    config: &DaemonConfig,
    state: AppState,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let router = create_router(state);
    let addr = config.socket_addr()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Daemon listening on {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown.cancelled().await;
        })
        .await?;

    Ok(())
}
```

**Testing:**

Tests in `tests/daemon_server_test.rs` using axum's test utilities:

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt; // for `oneshot`

#[tokio::test]
async fn test_health_check() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}
```

A `create_test_state()` helper function creates an `AppState` with an in-memory Database, SqliteGraphStore, and TokioMessageBus. This helper will be reused across all daemon API tests.

**Verification:**

Run: `cargo test daemon_server_test`
Expected: All tests pass

**Commit:** `feat(daemon): axum server with health endpoint and CORS middleware`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Create Project API endpoints

**Verifies:** P3b.AC4.1, P3b.AC4.2, P3b.AC4.3, P3b.AC4.4, P3b.AC4.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/projects.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/api/mod.rs` — add `pub mod projects;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — mount project routes
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_api_test.rs` — add project endpoint tests

**Implementation:**

`src/daemon/api/projects.rs`:

**Request/Response types:**

```rust
use serde::{Deserialize, Serialize};
use crate::project::Project;

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub path: String,
    pub registered_at: String,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        Self {
            id: p.id,
            name: p.name,
            path: p.path.display().to_string(),
            registered_at: p.registered_at.to_rfc3339(),
        }
    }
}
```

**Handlers:**

```rust
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use super::{AppState, ApiError};

/// GET /api/projects
pub async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProjectResponse>>, ApiError> {
    let projects = state.project_store.list().await?;
    Ok(Json(projects.into_iter().map(ProjectResponse::from).collect()))
}

/// POST /api/projects
pub async fn create_project(
    State(state): State<AppState>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ApiError> {
    let path = std::path::Path::new(&body.path);
    let canonical = path.canonicalize().map_err(|e| {
        ApiError::BadRequest(format!("Invalid path '{}': {}", body.path, e))
    })?;

    match state.project_store.add(&body.name, &canonical).await {
        Ok(project) => Ok((StatusCode::CREATED, Json(ProjectResponse::from(project)))),
        Err(e) if e.to_string().contains("UNIQUE constraint") => {
            Err(ApiError::Conflict(format!("Project '{}' already exists", body.name)))
        }
        Err(e) => Err(ApiError::Internal(e.to_string())),
    }
}

/// GET /api/projects/:id
pub async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, ApiError> {
    // Try by name first (more common in CLI usage), then by ID
    let project = state.project_store.get_by_name(&id).await?;
    let project = match project {
        Some(p) => Some(p),
        None => state.project_store.get_by_id(&id).await?,
    };
    match project {
        Some(p) => Ok(Json(ProjectResponse::from(p))),
        None => Err(ApiError::NotFound(format!("Project '{}' not found", id))),
    }
}

// Note: ProjectStore::get_by_id() may need to be added if it doesn't exist.
// It queries: SELECT * FROM projects WHERE id = ?1
// If adding this method is undesirable, document that the :id parameter
// accepts project names (the primary lookup key in CLI usage).

/// DELETE /api/projects/:id
pub async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let removed = state.project_store.remove(&id).await?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Project '{}' not found", id)))
    }
}
```

**Route mounting** in `server.rs`:

```rust
use crate::daemon::api::projects;

// Inside create_router():
Router::new()
    .route("/api/health", get(health_check))
    .route("/api/projects", get(projects::list_projects).post(projects::create_project))
    .route("/api/projects/{id}", get(projects::get_project).delete(projects::delete_project))
    .layer(cors)
    .with_state(state)
```

**Testing:**

Add to `tests/daemon_api_test.rs`:

- P3b.AC4.1: `GET /api/projects` on empty DB returns `[]`
- P3b.AC4.2: `POST /api/projects` with valid body returns 201 + project JSON
- P3b.AC4.1 (with data): After creating a project, `GET /api/projects` returns array with 1 element
- P3b.AC4.3: After creating, `GET /api/projects/{name}` returns the project
- P3b.AC4.3 (404): `GET /api/projects/nonexistent` returns 404
- P3b.AC4.4: After creating, `DELETE /api/projects/{name}` returns 204. Subsequent GET returns 404.
- P3b.AC4.5: Create project "foo", then POST again with name "foo" returns 409

All tests use `router.oneshot(request)` pattern from axum test utilities.

**Verification:**

Run: `cargo test daemon_api_test`
Expected: All tests pass

**Commit:** `feat(daemon): project CRUD REST API endpoints`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Wire daemon module and update daemon start command

**Verifies:** P3b.AC5.1, P3b.AC5.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` — uncomment submodule declarations
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — update daemon start to create AppState and call start_server

**Implementation:**

In `src/daemon/mod.rs`, uncomment:

```rust
pub mod api;
pub mod server;
// pub mod ws;  // Phase 3e
```

In `src/main.rs`, update the `DaemonAction::Start` handler to create `AppState` and start the server:

```rust
DaemonAction::Start { bind, port } => {
    let config = rustagent::daemon::DaemonConfig {
        bind_address: bind,
        port,
        ..config
    };

    if rustagent::daemon::is_daemon_running(&config)? {
        anyhow::bail!("Daemon is already running");
    }

    rustagent::daemon::write_pid_file(&config)?;

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let shutdown_clone = shutdown_token.clone();
    let cleanup_config = config.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("\nDaemon shutting down...");
        shutdown_clone.cancel();
    });

    // Open database
    let db_path = db_path()?;
    let database = db::Database::open(&db_path).await?;

    // Create shared dependencies
    let graph_store: std::sync::Arc<dyn GraphStore> =
        std::sync::Arc::new(rustagent::graph::store::SqliteGraphStore::new(database.clone()));
    let message_bus: std::sync::Arc<dyn rustagent::message::MessageBus> =
        std::sync::Arc::new(rustagent::message::TokioMessageBus::default());

    let state = rustagent::daemon::api::AppState::new(database, graph_store, message_bus);

    println!("Daemon listening on {}:{}", config.bind_address, config.port);

    // Start the HTTP server (blocks until shutdown)
    rustagent::daemon::server::start_server(&config, state, shutdown_token).await?;

    // Cleanup
    rustagent::daemon::remove_pid_file(&cleanup_config)?;
    println!("Daemon stopped.");
}
```

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- daemon start` (then Ctrl+C)
Expected: Starts server on 127.0.0.1:7400, responds to Ctrl+C with graceful shutdown

**Commit:** `feat(daemon): wire up HTTP server in daemon start command`

<!-- END_TASK_4 -->
<!-- END_SUBCOMPONENT_A -->
