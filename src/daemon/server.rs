use crate::daemon::api::AppState;
use crate::daemon::api::{agents, graph, projects, search};
use crate::daemon::ws;
use crate::daemon::DaemonConfig;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};

/// Create the axum Router with all routes and middleware
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health
        .route("/api/health", get(health_check))
        // Projects
        .route(
            "/api/projects",
            get(projects::list_projects).post(projects::create_project),
        )
        .route(
            "/api/projects/{id}",
            get(projects::get_project).delete(projects::delete_project),
        )
        // Goals
        .route(
            "/api/projects/{id}/goals",
            get(graph::list_goals).post(graph::create_goal),
        )
        // Nodes
        .route(
            "/api/nodes/{id}",
            get(graph::get_node).patch(graph::update_node),
        )
        .route("/api/nodes/{id}/children", post(graph::create_child))
        // Edges
        .route("/api/edges", post(graph::create_edge))
        .route("/api/edges/{id}", delete(graph::delete_edge))
        // Goal tree
        .route("/api/goals/{id}/tree", get(graph::get_goal_tree))
        // Tasks
        .route("/api/goals/{id}/tasks", get(graph::list_tasks))
        .route("/api/goals/{id}/tasks/ready", get(graph::list_ready_tasks))
        .route("/api/goals/{id}/tasks/next", get(graph::next_task))
        // Decisions
        .route(
            "/api/projects/{id}/decisions",
            get(graph::list_decisions),
        )
        .route(
            "/api/projects/{id}/decisions/history",
            get(graph::decisions_history),
        )
        .route(
            "/api/projects/{id}/decisions/export",
            post(graph::export_decisions),
        )
        // Search
        .route(
            "/api/projects/{id}/search",
            post(search::search_nodes),
        )
        // Sessions
        .route("/api/goals/{id}/sessions", get(graph::list_sessions))
        .route("/api/sessions/{id}", get(graph::get_session))
        // Agents
        .route("/api/goals/{id}/agents", get(agents::list_agents))
        // Graph import/export
        .route(
            "/api/projects/{id}/graph/export",
            get(graph::export_all_goals),
        )
        .route("/api/goals/{id}/export", get(graph::export_goal_toml))
        .route(
            "/api/projects/{id}/graph/import",
            post(graph::import_graph),
        )
        .route(
            "/api/projects/{id}/graph/diff",
            post(graph::diff_graph),
        )
        // WebSocket
        .route("/ws", get(ws::ws_handler))
        // Static file serving (fallback for non-API routes)
        .fallback(crate::daemon::static_files::static_handler)
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
