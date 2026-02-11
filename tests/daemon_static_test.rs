use axum::body::Body;
use axum::http::{Request, StatusCode};
use rustagent::daemon::api::AppState;
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

#[tokio::test]
async fn test_fallback_without_bundle_ui() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    // GET / should return fallback message (no bundle-ui feature)
    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["message"].as_str().unwrap().contains("not bundled"));
}

#[tokio::test]
async fn test_fallback_does_not_intercept_api() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    // /api/health should still work normally
    let request = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_fallback_unknown_path() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    // GET /some/unknown/path should hit fallback
    let request = Request::builder()
        .uri("/some/unknown/path")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["message"].as_str().unwrap().contains("not bundled"));
}
