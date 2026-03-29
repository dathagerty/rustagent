# Rustagent V2 Phase 3f: CLI Daemon Detection + Thin Client Mode

**Goal:** Update the CLI to auto-detect whether a daemon is running and route commands through the HTTP API when it is. When no daemon is running, commands execute locally (current behavior). This makes the CLI work seamlessly in both modes.

**Architecture:** The CLI checks for a running daemon by reading the PID file and making an HTTP health check (`GET /api/health`). If the daemon is reachable, the CLI becomes a thin client — each command is translated to the corresponding API call, and the response is formatted for terminal output. If unreachable, the CLI falls back to direct database access (current behavior). A `DaemonClient` struct wraps `reqwest` for API communication.

**Tech Stack:** Rust (edition 2024), reqwest 0.12 (already a dependency), clap 4.5, tokio 1.43

**Scope:** Phase 6 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3f.AC1: Daemon detection
- **P3f.AC1.1 Success:** `detect_daemon(config: &DaemonConfig)` returns `Some(DaemonClient)` if the daemon is running and healthy, `None` otherwise
- **P3f.AC1.2 Success:** Detection checks PID file first (fast), then confirms with HTTP health check (accurate)
- **P3f.AC1.3 Success:** If PID file exists but health check fails, returns `None` (daemon crashed but PID file is stale)

### P3f.AC2: DaemonClient
- **P3f.AC2.1 Success:** `DaemonClient` wraps `reqwest::Client` with the daemon's base URL
- **P3f.AC2.2 Success:** `DaemonClient::projects_list()` calls `GET /api/projects` and returns parsed JSON
- **P3f.AC2.3 Success:** `DaemonClient::health()` calls `GET /api/health` and returns true/false
- **P3f.AC2.4 Success:** API call errors are propagated as `anyhow::Error` with the HTTP status code in the message

### P3f.AC3: CLI routing
- **P3f.AC3.1 Success:** `project list` routes through daemon when running, falls back to direct DB when not
- **P3f.AC3.2 Success:** `project add` routes through daemon when running
- **P3f.AC3.3 Success:** `tasks list` routes through daemon when running
- **P3f.AC3.4 Success:** `search` routes through daemon when running
- **P3f.AC3.5 Success:** `status` routes through daemon when running
- **P3f.AC3.6 Success:** `run` always executes locally (orchestrator runs in-process, not through daemon API)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Create DaemonClient with daemon detection

**Verifies:** P3f.AC1.1, P3f.AC1.2, P3f.AC1.3, P3f.AC2.1, P3f.AC2.3, P3f.AC2.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/client.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs` — add `pub mod client;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_client_test.rs`

**Implementation:**

`src/daemon/client.rs`:

```rust
use crate::daemon::DaemonConfig;
use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;

/// HTTP client for communicating with a running daemon
#[derive(Clone)]
pub struct DaemonClient {
    client: Client,
    base_url: String,
}

impl DaemonClient {
    pub fn new(config: &DaemonConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: format!("http://{}:{}", config.bind_address, config.port),
        }
    }

    /// Check if the daemon is healthy
    pub async fn health(&self) -> bool {
        match self.client.get(format!("{}/api/health", self.base_url))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Generic GET request returning parsed JSON
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(resp.json().await?)
    }

    /// Generic POST request with JSON body returning parsed JSON
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(resp.json().await?)
    }

    /// Generic DELETE request
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.delete(&url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status.as_u16(), body);
        }
        Ok(())
    }
}

/// Detect if a daemon is running and return a client for it.
/// Checks PID file first (fast), then confirms with HTTP health check (accurate).
pub async fn detect_daemon(config: &DaemonConfig) -> Option<DaemonClient> {
    // Fast check: PID file exists and process is alive
    if !crate::daemon::is_daemon_running(config).unwrap_or(false) {
        return None;
    }

    // Accurate check: HTTP health endpoint responds
    let client = DaemonClient::new(config);
    if client.health().await {
        Some(client)
    } else {
        None
    }
}
```

**Testing:**

Tests in `tests/daemon_client_test.rs`:

- P3f.AC1.1: With no PID file, `detect_daemon` returns None
- P3f.AC1.3: Write a PID file with a dead PID. `detect_daemon` returns None (no HTTP health check passes)
- P3f.AC2.1: Construct `DaemonClient`, verify `base_url` is formatted correctly
- P3f.AC2.3: Start a test axum server with just the health endpoint. `DaemonClient::health()` returns true. Stop server. `health()` returns false.
- P3f.AC2.4: Send GET to a nonexistent path on the test server. Error message contains "404".

**Verification:**

Run: `cargo test daemon_client_test`
Expected: All tests pass

**Commit:** `feat(daemon): DaemonClient HTTP client and daemon detection`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Add DaemonClient API methods for CLI commands

**Verifies:** P3f.AC2.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/client.rs` — add typed API methods

**Implementation:**

Add convenience methods to `DaemonClient` that correspond to CLI commands:

```rust
use crate::daemon::api::projects::ProjectResponse;
use crate::graph::GraphNode;

impl DaemonClient {
    // Project operations
    pub async fn projects_list(&self) -> Result<Vec<ProjectResponse>> {
        self.get("/api/projects").await
    }

    pub async fn project_add(&self, name: &str, path: &str) -> Result<ProjectResponse> {
        self.post("/api/projects", &serde_json::json!({
            "name": name,
            "path": path,
        })).await
    }

    pub async fn project_get(&self, name: &str) -> Result<ProjectResponse> {
        self.get(&format!("/api/projects/{}", name)).await
    }

    pub async fn project_remove(&self, name: &str) -> Result<()> {
        self.delete(&format!("/api/projects/{}", name)).await
    }

    // Task operations
    pub async fn tasks_list(&self, goal_id: &str) -> Result<Vec<GraphNode>> {
        let path = format!("/api/goals/{}/tasks", goal_id);
        self.get(&path).await
    }

    pub async fn tasks_ready(&self, goal_id: &str) -> Result<Vec<GraphNode>> {
        self.get(&format!("/api/goals/{}/tasks/ready", goal_id)).await
    }

    pub async fn tasks_next(&self, goal_id: &str) -> Result<Option<GraphNode>> {
        self.get(&format!("/api/goals/{}/tasks/next", goal_id)).await
    }

    // Search
    pub async fn search(&self, project_id: &str, query: &str) -> Result<Vec<GraphNode>> {
        self.post(&format!("/api/projects/{}/search", project_id),
            &serde_json::json!({ "query": query })).await
    }

    // Status (goals for a project)
    pub async fn goals_list(&self, project_id: &str) -> Result<Vec<GraphNode>> {
        self.get(&format!("/api/projects/{}/goals", project_id)).await
    }
}
```

**Verification:**

Run: `cargo check`
Expected: Compiles cleanly

**Commit:** `feat(daemon): typed DaemonClient API methods for CLI commands`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Update CLI commands to route through daemon when available

**Verifies:** P3f.AC3.1, P3f.AC3.2, P3f.AC3.3, P3f.AC3.4, P3f.AC3.5, P3f.AC3.6

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add daemon detection and routing

**Implementation:**

The approach: at the top of `main()`, after parsing CLI args, detect the daemon. Store the result as `Option<DaemonClient>`. For each command handler, check if the client exists and use it if so.

```rust
// After parsing CLI args, before the match:
let daemon_config = rustagent::daemon::DaemonConfig::default();
let daemon_client = rustagent::daemon::client::detect_daemon(&daemon_config).await;
```

For each command that supports daemon routing, add an early check:

```rust
// Example for project list:
Commands::Project { action: ProjectAction::List } => {
    if let Some(ref client) = daemon_client {
        let projects = client.projects_list().await?;
        if projects.is_empty() {
            println!("No projects registered");
        } else {
            println!("{:<20} {:<10} {:<40}", "Name", "ID", "Path");
            println!("{}", "=".repeat(70));
            for proj in projects {
                println!("{:<20} {:<10} {:<40}", proj.name, proj.id, proj.path);
            }
        }
        return Ok(());
    }
    // ... existing direct-DB code unchanged ...
}
```

Commands that route through daemon:
- `project list/add/show/remove` — via project API
- `tasks list/ready/next` — via task API
- `search` — via search API
- `status` — via goals API + task counts

Commands that always run locally:
- `run` — orchestrator runs in-process (AC3.6)
- `daemon start/stop/status/logs` — direct process management
- `init` / `plan` — V1 commands

**Design note on `run` vs daemon orchestration:** The architecture doc says the daemon is "required for long-running orchestration." In this phase, `run` always executes locally with an in-process orchestrator. The daemon provides monitoring (WebSocket events, API status queries) but does not run orchestration itself. Future work could add `POST /api/projects/:id/goals` triggering daemon-managed orchestration, with `run` posting the goal to the daemon when one is detected and streaming progress via WebSocket. This is deferred — Phase 3 establishes the daemon infrastructure, not daemon-managed orchestration.

For the initial implementation, each command's daemon path calls the DaemonClient method and formats the output identically to the local path. This is somewhat verbose but keeps the local fallback working without changes.

**Alternative (refactored) approach:** Extract the display logic into shared functions that accept data (not how it was fetched). This reduces duplication. Example:

```rust
fn display_projects(projects: &[ProjectResponse]) {
    // ... formatting logic ...
}

// Daemon path:
let projects = client.projects_list().await?;
display_projects(&projects);

// Local path:
let projects: Vec<ProjectResponse> = store.list().await?.into_iter().map(Into::into).collect();
display_projects(&projects);
```

Prefer the refactored approach where practical, but don't force it where response types differ significantly between daemon and local paths.

**Testing:**

These are primarily integration/human-verified:
- P3f.AC3.1-5: Manual test with daemon running vs. not running
- P3f.AC3.6: Verify `run` command does NOT call daemon API (checked by code inspection and debug logs)

Automated test:
- Start a test server, create DaemonClient pointing to it. Call each client method. Verify responses match.

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Manual: Start daemon in one terminal, run CLI commands in another.

**Commit:** `feat(cli): auto-detect daemon and route commands through HTTP API`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
