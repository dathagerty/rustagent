use axum::body::Body;
use axum::http::{Request, StatusCode};
use rustagent::daemon::api::{ApiError, AppState, WsEvent};
use rustagent::db::Database;
use rustagent::graph::store::SqliteGraphStore;
use rustagent::message::TokioMessageBus;
use std::path::Path;
use std::sync::Arc;
use tower::ServiceExt;

async fn create_test_state() -> AppState {
    let db = Database::open(Path::new(":memory:")).await.unwrap();
    let graph_store: Arc<dyn rustagent::graph::store::GraphStore> =
        Arc::new(SqliteGraphStore::new(db.clone()));
    let message_bus: Arc<dyn rustagent::message::MessageBus> =
        Arc::new(TokioMessageBus::default());
    AppState::new(db, graph_store, message_bus)
}

// ===== AppState tests =====

#[tokio::test]
async fn test_app_state_clone() {
    let state = create_test_state().await;
    let cloned = state.clone();
    // Both reference the same Arc-wrapped resources
    assert!(Arc::ptr_eq(&state.graph_store, &cloned.graph_store));
    assert!(Arc::ptr_eq(&state.message_bus, &cloned.message_bus));
}

// ===== ApiError tests =====

#[tokio::test]
async fn test_api_error_not_found() {
    use axum::response::IntoResponse;
    let error = ApiError::NotFound("thing not found".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "not found");
    assert_eq!(json["message"], "thing not found");
}

#[tokio::test]
async fn test_api_error_bad_request() {
    use axum::response::IntoResponse;
    let error = ApiError::BadRequest("invalid input".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_api_error_conflict() {
    use axum::response::IntoResponse;
    let error = ApiError::Conflict("already exists".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_api_error_internal() {
    use axum::response::IntoResponse;
    let error = ApiError::Internal("something broke".to_string());
    let response = error.into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ===== Health endpoint =====

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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

// ===== WsEvent serialization =====

#[test]
fn test_ws_event_serialization() {
    let event = WsEvent::AgentSpawned {
        agent_id: "w-1".into(),
        profile: "coder".into(),
        goal_id: "ra-a3f8".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "agent_spawned");
    assert_eq!(json["agent_id"], "w-1");
}

#[test]
fn test_ws_event_agent_completed_serialization() {
    let event = WsEvent::AgentCompleted {
        agent_id: "w-1".into(),
        outcome_type: "blocked".into(),
        summary: "Missing dependency".into(),
        tokens_used: None,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "agent_completed");
    assert_eq!(json["outcome_type"], "blocked");
}

// ===== Project API =====

#[tokio::test]
async fn test_list_projects_empty() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = Request::builder()
        .uri("/api/projects")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_and_get_project() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    // Create project
    let body = serde_json::json!({
        "name": "test-project",
        "path": tmp.path().to_string_lossy(),
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "test-project");
    assert!(json["id"].as_str().unwrap().starts_with("ra-"));

    // Get project by name
    let request = Request::builder()
        .uri("/api/projects/test-project")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // List projects
    let request = Request::builder()
        .uri("/api/projects")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_get_nonexistent_project() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = Request::builder()
        .uri("/api/projects/nonexistent")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_project() {
    let tmp = tempfile::TempDir::new().unwrap();
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    // Create project
    let body = serde_json::json!({
        "name": "delete-me",
        "path": tmp.path().to_string_lossy(),
    });
    let request = Request::builder()
        .method("POST")
        .uri("/api/projects")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Delete project
    let request = Request::builder()
        .method("DELETE")
        .uri("/api/projects/delete-me")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deleted
    let request = Request::builder()
        .uri("/api/projects/delete-me")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
