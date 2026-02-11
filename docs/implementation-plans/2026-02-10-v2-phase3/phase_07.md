# Rustagent V2 Phase 3g: Static Asset Serving + bundle-ui Feature

**Goal:** Implement static file serving for the web UI — embedded assets via `rust-embed` when compiled with the `bundle-ui` feature, and a helpful fallback message when compiled without it. This makes the daemon a fully self-contained binary in release mode.

**Architecture:** Two serving modes controlled by a Cargo feature flag:

1. **Without `bundle-ui`** (default): The daemon does not serve UI assets. The `/*` fallback returns a JSON message directing the user to start the Vite dev server or build with `--features bundle-ui`.
2. **With `bundle-ui`**: `build.rs` runs `bun install && bun run build` in `web/`, then `rust-embed` compiles `web/dist/` into the binary. The daemon serves embedded assets with proper content types and SPA fallback to `index.html`.

**Tech Stack:** Rust (edition 2024), axum 0.8, rust-embed 8 (optional), build.rs

**Scope:** Phase 7 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3g.AC1: build.rs frontend compilation
- **P3g.AC1.1 Success:** `build.rs` only runs frontend build when `bundle-ui` feature is active
- **P3g.AC1.2 Success:** `build.rs` runs `bun install` then `bun run build` in `web/` directory
- **P3g.AC1.3 Success:** `build.rs` sets `cargo:rerun-if-changed` for `web/src` and `web/package.json`

### P3g.AC2: Embedded assets with rust-embed
- **P3g.AC2.1 Success:** `UiAssets` struct derives `rust_embed::Embed` with `folder = "web/dist/"`
- **P3g.AC2.2 Success:** `UiAssets` is only compiled when `bundle-ui` feature is active

### P3g.AC3: Fallback handler (without bundle-ui)
- **P3g.AC3.1 Success:** Without `bundle-ui`, any request to `/*` (not `/api/*` or `/ws`) returns a JSON message: `{"message": "UI not bundled. Run with --features bundle-ui or start the Vite dev server."}`
- **P3g.AC3.2 Success:** The fallback does not interfere with `/api/*` or `/ws` routes

### P3g.AC4: Static serving (with bundle-ui)
- **P3g.AC4.1 Success:** With `bundle-ui`, `GET /` returns `index.html` from embedded assets
- **P3g.AC4.2 Success:** `GET /assets/index.js` returns the bundled JS with correct content type
- **P3g.AC4.3 Success:** `GET /nonexistent-path` falls back to `index.html` (SPA routing)
- **P3g.AC4.4 Success:** Content types are inferred correctly (`.js` → `application/javascript`, `.css` → `text/css`, `.html` → `text/html`)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create build.rs for frontend compilation

**Verifies:** P3g.AC1.1, P3g.AC1.2, P3g.AC1.3

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/build.rs`

**Implementation:**

```rust
fn main() {
    #[cfg(feature = "bundle-ui")]
    {
        println!("cargo:rerun-if-changed=web/src");
        println!("cargo:rerun-if-changed=web/package.json");

        let web_dir = "web";

        // Check if web/ directory exists
        if !std::path::Path::new(web_dir).exists() {
            panic!(
                "web/ directory not found. The bundle-ui feature requires the web UI source. \
                 See docs/plans/v2-architecture.md Phase 4 for setup instructions."
            );
        }

        let status = std::process::Command::new("bun")
            .args(["install"])
            .current_dir(web_dir)
            .status()
            .expect("bun must be installed to build with bundle-ui feature");
        assert!(status.success(), "bun install failed");

        let status = std::process::Command::new("bun")
            .args(["run", "build"])
            .current_dir(web_dir)
            .status()
            .expect("bun run build failed");
        assert!(status.success(), "frontend build failed");
    }
}
```

**Verification:**

Run: `cargo build` (without bundle-ui)
Expected: Compiles without running bun

Run: `cargo build --features bundle-ui` (requires web/ directory — expected to fail until Phase 4)
Expected: Attempts to run bun, fails with clear message if web/ doesn't exist

**Commit:** `feat(build): build.rs for frontend compilation with bundle-ui feature`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Add embedded assets and static file serving

**Verifies:** P3g.AC2.1, P3g.AC2.2, P3g.AC3.1, P3g.AC3.2, P3g.AC4.1, P3g.AC4.2, P3g.AC4.3, P3g.AC4.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/static_files.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` — add `pub mod static_files;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/server.rs` — add fallback handler

**Implementation:**

`src/daemon/static_files.rs`:

```rust
use axum::http::{header, StatusCode, Uri};
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
            ).into_response()
        }
        None => {
            // SPA fallback: serve index.html for unrecognized paths
            match UiAssets::get("index.html") {
                Some(file) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html".to_string())],
                    file.data.to_vec(),
                ).into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    "index.html not found in embedded assets",
                ).into_response(),
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
```

Note: Add `mime_guess` as a dependency (only needed with bundle-ui):

```toml
mime_guess = { version = "2", optional = true }

[features]
bundle-ui = ["dep:rust-embed", "dep:mime_guess"]
```

**Fallback handler** in `server.rs`:

```rust
use crate::daemon::static_files;

// Add to create_router() — MUST be the last route (fallback):
Router::new()
    // ... all /api/* and /ws routes first ...
    .fallback(static_files::static_handler)
    .layer(cors)
    .with_state(state)
```

**Testing:**

Tests in `tests/daemon_static_test.rs`:

Without `bundle-ui` (default):
- P3g.AC3.1: `GET /` returns JSON with "UI not bundled" message
- P3g.AC3.2: `GET /api/health` still returns health response (not intercepted by fallback)

With `bundle-ui` (conditional test, only runs if feature is active):
- P3g.AC4.1-4: These require the web/ directory to be built. Mark tests with `#[cfg(feature = "bundle-ui")]`.

```rust
#[tokio::test]
async fn test_fallback_without_bundle_ui() {
    let state = create_test_state().await;
    let router = rustagent::daemon::server::create_router(state);

    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["message"].as_str().unwrap().contains("not bundled"));

    // Verify /api routes still work
    let request = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
```

**Verification:**

Run: `cargo test daemon_static_test`
Expected: All tests pass (without bundle-ui feature)

**Commit:** `feat(daemon): static file serving with SPA fallback and bundle-ui feature`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Wire complete daemon module

**Verifies:** P3b.AC5.2 (final wiring)

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` — ensure all submodules are exported

**Implementation:**

Final state of `src/daemon/mod.rs`:

```rust
pub mod api;
pub mod client;
pub mod server;
pub mod static_files;
pub mod ws;

// ... DaemonConfig, PID file functions from Phase 3a ...
```

Verify all submodules compile together:

```rust
// Run full build
cargo build
cargo test
```

**Verification:**

Run: `cargo build`
Expected: Full daemon module compiles cleanly

Run: `cargo test`
Expected: All existing + new tests pass

**Commit:** `feat(daemon): wire complete daemon module with all submodules`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
