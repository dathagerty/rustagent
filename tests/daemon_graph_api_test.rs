use axum::body::Body;
use axum::http::{Request, StatusCode};
use rustagent::daemon::api::AppState;
use rustagent::db::Database;
use rustagent::graph::store::SqliteGraphStore;
use rustagent::message::TokioMessageBus;
use rustagent::project::ProjectStore;
use std::path::Path;
use std::sync::Arc;
use tower::ServiceExt;

async fn create_test_state() -> AppState {
    let db = Database::open(Path::new(":memory:")).await.unwrap();
    let graph_store: Arc<dyn rustagent::graph::store::GraphStore> =
        Arc::new(SqliteGraphStore::new(db.clone()));
    let message_bus: Arc<dyn rustagent::message::MessageBus> = Arc::new(TokioMessageBus::default());
    AppState::new(db, graph_store, message_bus)
}

/// Creates a project "proj-1" so that foreign key constraints pass.
async fn ensure_project(state: &AppState) {
    let ps = ProjectStore::new(state.db.clone());
    // Insert directly — the project API would also work, but this is simpler
    ps.add("proj-1", Path::new("/tmp/test-project"))
        .await
        .unwrap();
}

fn json_request(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let body = match body {
        Some(v) => Body::from(serde_json::to_string(&v).unwrap()),
        None => Body::empty(),
    };
    builder.body(body).unwrap()
}

async fn response_json(response: axum::http::Response<Body>) -> serde_json::Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

// ===== Goal endpoints =====

#[tokio::test]
async fn test_create_and_list_goals() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create a goal
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Build auth system",
            "description": "Implement authentication"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = response_json(response).await;
    assert_eq!(json["node_type"], "goal");
    assert_eq!(json["status"], "active");
    // project_id is the resolved ra-XXXX ID, not the project name
    assert!(json["project_id"].as_str().unwrap().starts_with("ra-"));
    assert!(json["id"].as_str().unwrap().starts_with("ra-"));

    // List goals
    let request = json_request("GET", "/api/projects/proj-1/goals", None);
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let json = response_json(response).await;
    assert_eq!(json.as_array().unwrap().len(), 1);
}

// ===== Node endpoints =====

#[tokio::test]
async fn test_get_node_not_found() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = json_request("GET", "/api/nodes/nonexistent", None);
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_child_node() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal first
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Test goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    // Create child task
    let request = json_request(
        "POST",
        &format!("/api/nodes/{}/children", goal_id),
        Some(serde_json::json!({
            "node_type": "task",
            "title": "Implement login",
            "description": "Create login endpoint"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let json = response_json(response).await;
    assert_eq!(json["node_type"], "task");
    assert!(
        json["id"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{}.", goal_id))
    );
}

#[tokio::test]
async fn test_update_node_status() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Test goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    // Update status to completed
    let request = json_request(
        "PATCH",
        &format!("/api/nodes/{}", goal_id),
        Some(serde_json::json!({ "status": "completed" })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let json = response_json(response).await;
    assert_eq!(json["status"], "completed");
}

#[tokio::test]
async fn test_update_node_invalid_status() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Test goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    // Try invalid status for goal
    let request = json_request(
        "PATCH",
        &format!("/api/nodes/{}", goal_id),
        Some(serde_json::json!({ "status": "ready" })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ===== Edge endpoints =====

#[tokio::test]
async fn test_create_and_delete_edge() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    // Create two child tasks
    let request = json_request(
        "POST",
        &format!("/api/nodes/{}/children", goal_id),
        Some(serde_json::json!({
            "node_type": "task",
            "title": "Task A",
            "description": "First task"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let task_a = response_json(response).await;
    let task_a_id = task_a["id"].as_str().unwrap();

    let request = json_request(
        "POST",
        &format!("/api/nodes/{}/children", goal_id),
        Some(serde_json::json!({
            "node_type": "task",
            "title": "Task B",
            "description": "Second task"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let task_b = response_json(response).await;
    let task_b_id = task_b["id"].as_str().unwrap();

    // Create edge
    let request = json_request(
        "POST",
        "/api/edges",
        Some(serde_json::json!({
            "edge_type": "depends_on",
            "from_node": task_b_id,
            "to_node": task_a_id
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let edge = response_json(response).await;
    let edge_id = edge["id"].as_str().unwrap();

    // Delete edge
    let request = json_request("DELETE", &format!("/api/edges/{}", edge_id), None);
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_create_edge_nonexistent_node() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = json_request(
        "POST",
        "/api/edges",
        Some(serde_json::json!({
            "edge_type": "depends_on",
            "from_node": "nonexistent-1",
            "to_node": "nonexistent-2"
        })),
    );
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ===== Goal tree =====

#[tokio::test]
async fn test_goal_tree() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal with child
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    let request = json_request(
        "POST",
        &format!("/api/nodes/{}/children", goal_id),
        Some(serde_json::json!({
            "node_type": "task",
            "title": "Child task",
            "description": "A task"
        })),
    );
    router.clone().oneshot(request).await.unwrap();

    // Get tree
    let request = json_request("GET", &format!("/api/goals/{}/tree", goal_id), None);
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let json = response_json(response).await;
    assert!(json["nodes"].as_array().unwrap().len() >= 2); // goal + child
    assert!(!json["edges"].as_array().unwrap().is_empty()); // Contains edge
}

#[tokio::test]
async fn test_goal_tree_not_found() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = json_request("GET", "/api/goals/nonexistent/tree", None);
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ===== Task views =====

#[tokio::test]
async fn test_list_tasks_for_goal() {
    let state = create_test_state().await;
    ensure_project(&state).await;
    let router = rustagent::daemon::server::create_router(state);

    // Create goal + task child
    let request = json_request(
        "POST",
        "/api/projects/proj-1/goals",
        Some(serde_json::json!({
            "title": "Goal",
            "description": "A goal"
        })),
    );
    let response = router.clone().oneshot(request).await.unwrap();
    let goal = response_json(response).await;
    let goal_id = goal["id"].as_str().unwrap();

    let request = json_request(
        "POST",
        &format!("/api/nodes/{}/children", goal_id),
        Some(serde_json::json!({
            "node_type": "task",
            "title": "My task",
            "description": "A task"
        })),
    );
    router.clone().oneshot(request).await.unwrap();

    // List tasks
    let request = json_request("GET", &format!("/api/goals/{}/tasks", goal_id), None);
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let json = response_json(response).await;
    let tasks = json.as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["node_type"], "task");
}
