use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};

/// Embedded UI assets (only available with bundle-ui feature)
#[cfg(feature = "bundle-ui")]
#[derive(rust_embed::Embed)]
#[folder = "web/dist/"]
struct UiAssets;

/// Serve static files from embedded assets, or return a fallback message
pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    serve_asset(path)
}

#[cfg(feature = "bundle-ui")]
fn serve_asset(path: &str) -> Response {
    use axum::http::header;

    match UiAssets::get(path) {
        Some(file) => {
            let content_type = mime_guess::from_path(path)
                .first_or_octet_stream()
                .as_ref()
                .to_string();

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type)],
                file.data.to_vec(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for unrecognized paths
            match UiAssets::get("index.html") {
                Some(file) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    file.data.to_vec(),
                )
                    .into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    "index.html not found in embedded assets",
                )
                    .into_response(),
            }
        }
    }
}

#[cfg(not(feature = "bundle-ui"))]
fn serve_asset(_path: &str) -> Response {
    let body = serde_json::json!({
        "message": "UI not bundled. Run with --features bundle-ui or start the Vite dev server."
    });
    (StatusCode::OK, axum::Json(body)).into_response()
}
