use rustagent::daemon::DaemonConfig;
use rustagent::daemon::client::{DaemonClient, detect_daemon};
use std::path::Path;
use tempfile::TempDir;

// ===== DaemonClient construction =====

#[test]
fn test_daemon_client_base_url() {
    let config = DaemonConfig::default();
    let client = DaemonClient::new(&config);
    // The client should be constructable — we can't directly inspect base_url
    // but we verify it doesn't panic and the config values are used
    let _ = client;
}

#[test]
fn test_daemon_client_custom_config() {
    let config = DaemonConfig {
        bind_address: "0.0.0.0".to_string(),
        port: 9999,
        ..DaemonConfig::default()
    };
    let _client = DaemonClient::new(&config);
}

// ===== detect_daemon =====

#[tokio::test]
async fn test_detect_daemon_no_pid_file() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("nonexistent.pid"),
        ..DaemonConfig::default()
    };
    // No PID file → None
    assert!(detect_daemon(&config).await.is_none());
}

#[tokio::test]
async fn test_detect_daemon_dead_pid() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };
    // Write a PID that doesn't exist
    std::fs::write(&config.pid_file, "99999999").unwrap();
    // Dead PID → None
    assert!(detect_daemon(&config).await.is_none());
}

#[tokio::test]
async fn test_detect_daemon_live_pid_no_server() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        port: 19876, // Use a port that nothing listens on
        ..DaemonConfig::default()
    };
    // Write our own PID (alive) but no server on that port
    rustagent::daemon::write_pid_file(&config).unwrap();
    // PID alive but health check fails → None
    assert!(detect_daemon(&config).await.is_none());
}

// ===== DaemonClient health check against a real server =====

#[tokio::test]
async fn test_health_with_test_server() {
    let db = rustagent::db::Database::open(Path::new(":memory:"))
        .await
        .unwrap();
    let graph_store: std::sync::Arc<dyn rustagent::graph::store::GraphStore> =
        std::sync::Arc::new(rustagent::graph::store::SqliteGraphStore::new(db.clone()));
    let message_bus: std::sync::Arc<dyn rustagent::message::MessageBus> =
        std::sync::Arc::new(rustagent::message::TokioMessageBus::default());
    let state = rustagent::daemon::api::AppState::new(db, graph_store, message_bus);
    let router = rustagent::daemon::server::create_router(state);

    // Bind to a random available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Start the server
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let config = DaemonConfig {
        port,
        ..DaemonConfig::default()
    };
    let client = DaemonClient::new(&config);

    // Health check should succeed
    assert!(client.health().await);

    // Projects list should return empty array
    let projects: Vec<rustagent::daemon::api::projects::ProjectResponse> =
        client.get("/api/projects").await.unwrap();
    assert!(projects.is_empty());

    // GET to nonexistent API path hits the fallback handler (returns 200 with fallback JSON)
    let fallback: serde_json::Value = client.get("/api/nonexistent").await.unwrap();
    assert!(
        fallback["message"]
            .as_str()
            .unwrap()
            .contains("not bundled")
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_health_no_server() {
    let config = DaemonConfig {
        port: 19877, // Nothing listening
        ..DaemonConfig::default()
    };
    let client = DaemonClient::new(&config);
    assert!(!client.health().await);
}
