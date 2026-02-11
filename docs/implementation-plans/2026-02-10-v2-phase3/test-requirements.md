# Test Requirements for V2 Phase 3

This document maps every acceptance criterion from Phase 3a through Phase 3g to specific automated tests or documented human verification steps. Each criterion is traced to the implementation plan task that produces it and the test file where verification lives.

The V2 Phase 3 architecture covers Daemon + HTTP API: dependencies, daemon lifecycle, REST API endpoints (projects, graph, tasks, decisions, search, sessions, agents, import/export), WebSocket event streaming, CLI thin client mode, and static asset serving.

---

## Phase 3a: Dependencies + Daemon Lifecycle

### P3a.AC1: New dependencies compile

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3a.AC1.1 | build | N/A | `cargo check` succeeds after adding axum 0.8 with ws feature. Verified by CI build. |
| P3a.AC1.2 | build | N/A | `cargo check` succeeds after adding tower 0.5. |
| P3a.AC1.3 | build | N/A | `cargo check` succeeds after adding tower-http 0.6 with cors feature. |
| P3a.AC1.4 | build | N/A | `cargo check --features bundle-ui` succeeds after adding rust-embed 8 as optional dep. |

### P3a.AC2: DaemonConfig

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3a.AC2.1 | unit | `tests/daemon_test.rs` | Construct `DaemonConfig::default()`. Verify bind_address="127.0.0.1", port=7400, pid_file contains "rustagent.pid", log_dir contains "logs". |
| P3a.AC2.2 | unit | `tests/daemon_test.rs` | Same as AC2.1. |
| P3a.AC2.3 | unit | `tests/daemon_test.rs` | `DaemonConfig::default().socket_addr()` returns `Ok(addr)` where `addr.port() == 7400`. |

### P3a.AC3: PID file management

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3a.AC3.1 | unit | `tests/daemon_test.rs` | Create config with tempdir PID path. `write_pid_file()`. File exists and contains `std::process::id()`. |
| P3a.AC3.2 | unit | `tests/daemon_test.rs` | After write, `read_pid_file()` returns `Some(pid)`. Without write, returns `None`. |
| P3a.AC3.3 | unit | `tests/daemon_test.rs` | After write, `remove_pid_file()` succeeds. File no longer exists. Removing nonexistent file also succeeds. |
| P3a.AC3.4 | unit | `tests/daemon_test.rs` | Write current process PID. `is_daemon_running()` returns true. |
| P3a.AC3.5 | unit | `tests/daemon_test.rs` | Write PID 99999999 (dead process). `is_daemon_running()` returns false. |

### P3a.AC4: Daemon CLI subcommand

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3a.AC4.1 | human | N/A | `cargo run -- daemon start` starts the daemon in foreground on 127.0.0.1:7400. |
| P3a.AC4.2 | human | N/A | `cargo run -- daemon stop` sends SIGTERM to daemon PID. |
| P3a.AC4.3 | human | N/A | `cargo run -- daemon status` prints "Daemon is running (PID X)" or "Daemon is not running." |
| P3a.AC4.4 | human | N/A | Starting when already running prints error. |
| P3a.AC4.5 | human | N/A | Stopping when not running prints "No daemon is running." |
| P3a.AC4.6 | human | N/A | `cargo run -- daemon logs` tails the most recent log file from the log directory. |

**Implementation task:** Phase 3a, Tasks 1-3.

---

## Phase 3b: HTTP Server Foundation + Project API

### P3b.AC1: AppState

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3b.AC1.1 | unit | `tests/daemon_api_test.rs` | Construct `AppState::new()` with in-memory DB, SqliteGraphStore, TokioMessageBus. Access all fields. |
| P3b.AC1.2 | unit | `tests/daemon_api_test.rs` | Clone AppState. Both copies have same Arc-wrapped inner values. |

### P3b.AC2: Server startup

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3b.AC2.1 | integration | `tests/daemon_server_test.rs` | `create_router(state)` returns a Router. Send a request through it via `oneshot`. |
| P3b.AC2.2 | integration | `tests/daemon_server_test.rs` | Send request with `Origin: http://example.com` header. Response includes CORS headers. |
| P3b.AC2.3 | integration | `tests/daemon_server_test.rs` | `GET /api/health` returns 200 with `{"status": "ok"}`. |

### P3b.AC3: API error handling

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3b.AC3.1 | unit | `tests/daemon_api_test.rs` | Construct each ApiError variant, `into_response()` returns correct StatusCode. |
| P3b.AC3.2 | unit | `tests/daemon_api_test.rs` | `ApiError::NotFound` response body has `"error": "not found"`. |
| P3b.AC3.3 | unit | `tests/daemon_api_test.rs` | `ApiError::BadRequest` response body has `"error": "bad request"`. |
| P3b.AC3.4 | unit | `tests/daemon_api_test.rs` | `ApiError::Internal` response body has `"error": "internal error"`. |

### P3b.AC4: Project API

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3b.AC4.1 | integration | `tests/daemon_api_test.rs` | `GET /api/projects` on empty DB returns `200` with `[]`. After creating a project, returns array with 1 element. |
| P3b.AC4.2 | integration | `tests/daemon_api_test.rs` | `POST /api/projects` with `{"name":"test","path":"/tmp"}` returns 201 with ProjectResponse JSON. |
| P3b.AC4.3 | integration | `tests/daemon_api_test.rs` | After creating "test", `GET /api/projects/test` returns the project. `GET /api/projects/nonexistent` returns 404. |
| P3b.AC4.4 | integration | `tests/daemon_api_test.rs` | After creating, `DELETE /api/projects/test` returns 204. Subsequent GET returns 404. |
| P3b.AC4.5 | integration | `tests/daemon_api_test.rs` | Create "test", then `POST /api/projects` with name "test" again returns 409. |

### P3b.AC5: Server integration with daemon

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3b.AC5.1 | integration | `tests/daemon_server_test.rs` | `start_server` with CancellationToken. Cancel token, verify server stops. |
| P3b.AC5.2 | human | N/A | `cargo run -- daemon start` creates full AppState and serves health endpoint. |

**Implementation tasks:** Phase 3b, Tasks 1-4.

---

## Phase 3c: Graph Node/Edge API

### P3c.AC1: Goal endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3c.AC1.1 | integration | `tests/daemon_graph_api_test.rs` | Create project+goal. `GET /api/projects/{id}/goals` returns array with the goal. |
| P3c.AC1.2 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/projects/{id}/goals` with title/description returns 201 with GraphNode JSON. |
| P3c.AC1.3 | integration | `tests/daemon_graph_api_test.rs` | Created goal has `node_type: "goal"`, `status: "active"`, correct `project_id`. |

### P3c.AC2: Node endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3c.AC2.1 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/nodes/{id}` returns NodeWithEdges JSON with node, incoming_edges, outgoing_edges. |
| P3c.AC2.2 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/nodes/nonexistent` returns 404. |
| P3c.AC2.3 | integration | `tests/daemon_graph_api_test.rs` | `PATCH /api/nodes/{id}` with `{"status":"completed"}` updates status. |
| P3c.AC2.4 | integration | `tests/daemon_graph_api_test.rs` | `PATCH /api/nodes/{id}` with `{"title":"new"}` updates title. |
| P3c.AC2.5 | integration | `tests/daemon_graph_api_test.rs` | `PATCH` goal with `{"status":"ready"}` returns 400 (Ready not valid for Goal). |
| P3c.AC2.6 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/nodes/{id}/children` creates child with hierarchical ID. |

### P3c.AC3: Edge endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3c.AC3.1 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/edges` with valid from/to returns 201 with edge JSON. |
| P3c.AC3.2 | integration | `tests/daemon_graph_api_test.rs` | `DELETE /api/edges/{id}` returns 204. |
| P3c.AC3.3 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/edges` with nonexistent from_node returns 400. |

### P3c.AC4: Goal tree endpoint

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3c.AC4.1 | integration | `tests/daemon_graph_api_test.rs` | Create goal with children. `GET /api/goals/{id}/tree` returns all descendants. |
| P3c.AC4.2 | integration | `tests/daemon_graph_api_test.rs` | Response has `nodes` and `edges` arrays. |
| P3c.AC4.3 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/goals/nonexistent/tree` returns 404. |

**Implementation tasks:** Phase 3c, Tasks 1-3.

---

## Phase 3d: Task/Decision Views + Search + Sessions + Agents + Import/Export API

### P3d.AC1: Task view endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC1.1 | integration | `tests/daemon_graph_api_test.rs` | Create goal+tasks. `GET /api/goals/{id}/tasks` returns task nodes only. |
| P3d.AC1.2 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/goals/{id}/tasks/ready` returns only Ready tasks. |
| P3d.AC1.3 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/goals/{id}/tasks/next` returns highest-priority Ready task or `null`. |

### P3d.AC2: Decision view endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC2.1 | integration | `tests/daemon_graph_api_test.rs` | Create active decision. `GET /api/projects/{id}/decisions` returns it. |
| P3d.AC2.2 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/projects/{id}/decisions/history` returns `DecisionHistory` with `nodes` (Decision, Option, Outcome, Revisit) and `edges` arrays. Create a decided decision with chosen/rejected options and verify all appear. |
| P3d.AC2.3 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/projects/{id}/decisions/export` returns file path list. |

### P3d.AC3: Search endpoint

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC3.1 | integration | `tests/daemon_search_api_test.rs` | Create nodes with "authentication" in title. `POST /api/projects/{id}/search` with query "auth" returns matches. |
| P3d.AC3.2 | integration | `tests/daemon_search_api_test.rs` | Search with `node_type: "task"` returns only task nodes. |
| P3d.AC3.3 | integration | `tests/daemon_search_api_test.rs` | Search with `limit: 3` returns at most 3 results. |

### P3d.AC4: Session endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC4.1 | integration | `tests/daemon_graph_api_test.rs` | Create session. `GET /api/goals/{id}/sessions` returns it. |
| P3d.AC4.2 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/sessions/{id}` returns session with handoff_notes. |

### P3d.AC5: Agent status endpoint

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC5.1 | integration | `tests/daemon_graph_api_test.rs` | Create InProgress task with assigned_to. `GET /api/goals/{id}/agents` returns the agent. Empty when no InProgress tasks. |

### P3d.AC6: Graph import/export endpoints

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3d.AC6.1 | integration | `tests/daemon_graph_api_test.rs` | Create project+goal+tasks. `GET /api/projects/{id}/graph/export` returns TOML array. |
| P3d.AC6.2 | integration | `tests/daemon_graph_api_test.rs` | `GET /api/goals/{id}/export` returns single goal TOML. |
| P3d.AC6.3 | integration | `tests/daemon_graph_api_test.rs` | Export then import (round-trip). `POST /api/projects/{id}/graph/import` returns import result. |
| P3d.AC6.4 | integration | `tests/daemon_graph_api_test.rs` | `POST /api/projects/{id}/graph/diff` returns diff result. |

**Implementation tasks:** Phase 3d, Tasks 1-6.

---

## Phase 3e: WebSocket Event Streaming

### P3e.AC1: WsEvent types

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3e.AC1.1 | unit | `tests/daemon_ws_test.rs` | Construct each of the 9 WsEvent variants. Compiles with correct field names. |
| P3e.AC1.2 | unit | `tests/daemon_ws_test.rs` | Each variant carries correct fields (verified by construction in AC1.1). |
| P3e.AC1.3 | unit | `tests/daemon_ws_test.rs` | Serialize each variant. JSON has `"type"` field matching snake_case variant name. |

### P3e.AC2: WebSocket handler

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3e.AC2.1 | integration | `tests/daemon_ws_test.rs` | Connect to `ws://localhost:{port}/ws`. Upgrade succeeds. Uses tokio-tungstenite. |
| P3e.AC2.2 | integration | `tests/daemon_ws_test.rs` | Send WsEvent through ws_tx. Client receives JSON text message. |
| P3e.AC2.3 | integration | `tests/daemon_ws_test.rs` | Two clients connected. Both receive event sent through ws_tx. |
| P3e.AC2.4 | integration | `tests/daemon_ws_test.rs` | Connect and disconnect client. Send another event. No server errors. |
| P3e.AC2.5 | deferred | N/A | Heartbeat/ping interval is a tuning concern. Deferred to operational testing. |

### P3e.AC3: MessageBus-to-WsEvent bridge

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3e.AC3.1 | integration | `tests/daemon_ws_test.rs` | Start WsBroadcaster. Subscribe to ws_tx. Send WorkerMessage via MessageBus. Receive WsEvent. |
| P3e.AC3.2 | integration | `tests/daemon_ws_test.rs` | `ProgressReport` maps to `AgentProgress`. |
| P3e.AC3.3 | integration | `tests/daemon_ws_test.rs` | `TaskCompleted` maps to `AgentCompleted` with `outcome_type: "completed"`. |
| P3e.AC3.4 | integration | `tests/daemon_ws_test.rs` | `NodeCreated` maps to `NodeCreated` WsEvent with full `GraphNode`. |
| P3e.AC3.5 | integration | `tests/daemon_ws_test.rs` | `TaskBlocked` maps to `AgentCompleted` with `outcome_type: "blocked"`. |

### P3e.AC4: Deferred event emission

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3e.AC4.1 | deferred | N/A | `NodeStatusChanged` emission requires GraphStore mutation hooks. Wired with daemon-orchestrator integration. |
| P3e.AC4.2 | deferred | N/A | `EdgeCreated` emission requires GraphStore mutation hooks. |
| P3e.AC4.3 | deferred | N/A | `SessionEnded` emission requires orchestrator to emit via ws_tx. |
| P3e.AC4.4 | deferred | N/A | `ToolExecution` emission requires AgentRuntime hook + new WorkerMessage variant. |
| P3e.AC4.5 | deferred | N/A | `OrchestratorStateChanged` emission requires orchestrator state machine hooks. |

**Implementation tasks:** Phase 3e, Tasks 1-3.

---

## Phase 3f: CLI Daemon Detection + Thin Client Mode

### P3f.AC1: Daemon detection

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3f.AC1.1 | unit | `tests/daemon_client_test.rs` | No PID file: `detect_daemon()` returns None. |
| P3f.AC1.2 | integration | `tests/daemon_client_test.rs` | PID file + healthy server: returns `Some(DaemonClient)`. |
| P3f.AC1.3 | unit | `tests/daemon_client_test.rs` | Dead PID in file + no server: returns None. |

### P3f.AC2: DaemonClient

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3f.AC2.1 | unit | `tests/daemon_client_test.rs` | `DaemonClient::new()` has correct `base_url`. |
| P3f.AC2.2 | integration | `tests/daemon_client_test.rs` | Start test server. `projects_list()` returns parsed JSON. |
| P3f.AC2.3 | integration | `tests/daemon_client_test.rs` | Start test server. `health()` returns true. Stop server. `health()` returns false. |
| P3f.AC2.4 | integration | `tests/daemon_client_test.rs` | Request to nonexistent endpoint. Error message contains "404". |

### P3f.AC3: CLI routing

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3f.AC3.1 | human | N/A | Start daemon. Run `cargo run -- project list`. Output comes from daemon API. |
| P3f.AC3.2 | human | N/A | Start daemon. Run `cargo run -- project add test /tmp`. Daemon processes the request. |
| P3f.AC3.3 | human | N/A | Start daemon. Run `cargo run -- tasks list --project test`. Output from daemon. |
| P3f.AC3.4 | human | N/A | Start daemon. Run `cargo run -- search "auth" --project test`. Output from daemon. |
| P3f.AC3.5 | human | N/A | Start daemon. Run `cargo run -- status --project test`. Output from daemon. |
| P3f.AC3.6 | human | N/A | `cargo run -- run "goal"` executes locally even when daemon is running. |

**Implementation tasks:** Phase 3f, Tasks 1-3.

---

## Phase 3g: Static Asset Serving + bundle-ui Feature

### P3g.AC1: build.rs frontend compilation

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3g.AC1.1 | build | N/A | `cargo build` without `bundle-ui` feature does not run bun. |
| P3g.AC1.2 | build | N/A | `cargo build --features bundle-ui` runs bun install and bun run build (requires web/ dir). |
| P3g.AC1.3 | build | N/A | build.rs has `cargo:rerun-if-changed=web/src` and `web/package.json`. |

### P3g.AC2: Embedded assets with rust-embed

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3g.AC2.1 | build | N/A | `UiAssets` struct compiles with `bundle-ui` feature. |
| P3g.AC2.2 | build | N/A | Without `bundle-ui`, `UiAssets` is not compiled (no compilation error for missing web/dist/). |

### P3g.AC3: Fallback handler (without bundle-ui)

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3g.AC3.1 | integration | `tests/daemon_static_test.rs` | `GET /` returns JSON with "UI not bundled" message. |
| P3g.AC3.2 | integration | `tests/daemon_static_test.rs` | `GET /api/health` still returns health response (fallback doesn't intercept API routes). |

### P3g.AC4: Static serving (with bundle-ui)

| AC | Type | Test File | Description |
|----|------|-----------|-------------|
| P3g.AC4.1 | conditional | `tests/daemon_static_test.rs` | `#[cfg(feature = "bundle-ui")]`: `GET /` returns index.html. |
| P3g.AC4.2 | conditional | `tests/daemon_static_test.rs` | `#[cfg(feature = "bundle-ui")]`: `GET /assets/index.js` returns JS with correct content type. |
| P3g.AC4.3 | conditional | `tests/daemon_static_test.rs` | `#[cfg(feature = "bundle-ui")]`: `GET /nonexistent` falls back to index.html. |
| P3g.AC4.4 | conditional | `tests/daemon_static_test.rs` | `#[cfg(feature = "bundle-ui")]`: Content types inferred correctly. |

**Implementation tasks:** Phase 3g, Tasks 1-3.

---

## Human Verification Required

The following acceptance criteria cannot be fully automated because they depend on daemon process management, CLI output formatting, or cross-process communication.

| AC | Phase | Reason | Verification Approach |
|----|-------|--------|----------------------|
| P3a.AC4.1 | 3a | Daemon foreground start requires manual Ctrl+C testing | Run `cargo run -- daemon start`. Verify output shows listening address. Ctrl+C stops cleanly. |
| P3a.AC4.2 | 3a | Cross-process signal sending | Start daemon in one terminal. Run `cargo run -- daemon stop` in another. Verify daemon stops. |
| P3a.AC4.3 | 3a | CLI output presentation | Run `cargo run -- daemon status` with and without daemon running. |
| P3a.AC4.4 | 3a | Error output on duplicate start | Start daemon, then try to start again. |
| P3a.AC4.5 | 3a | Error output when not running | Run `cargo run -- daemon stop` with no daemon. |
| P3a.AC4.6 | 3a | Log file tailing | Run `cargo run -- daemon logs`. Verify most recent log file is displayed. Test `--follow` flag with a running daemon. |
| P3b.AC5.2 | 3b | Full daemon start wiring | `cargo run -- daemon start` then `curl localhost:7400/api/health` returns ok. |
| P3f.AC3.1-6 | 3f | CLI thin client behavior requires live daemon | Start daemon. Run CLI commands. Verify output is identical to direct mode. |

---

## Test File Summary

| Test File | Phase | Acceptance Criteria Covered |
|-----------|-------|-----------------------------|
| `tests/daemon_test.rs` | 3a | P3a.AC2.1-3, P3a.AC3.1-5 |
| `tests/daemon_api_test.rs` | 3b | P3b.AC1.1-2, P3b.AC3.1-4, P3b.AC4.1-5 |
| `tests/daemon_server_test.rs` | 3b | P3b.AC2.1-3, P3b.AC5.1 |
| `tests/daemon_graph_api_test.rs` | 3c, 3d | P3c.AC1-4, P3d.AC1-2, P3d.AC4-6 |
| `tests/daemon_search_api_test.rs` | 3d | P3d.AC3.1-3 |
| `tests/daemon_ws_test.rs` | 3e | P3e.AC1.1-3, P3e.AC2.1-4, P3e.AC3.1-4 |
| `tests/daemon_client_test.rs` | 3f | P3f.AC1.1-3, P3f.AC2.1-4 |
| `tests/daemon_static_test.rs` | 3g | P3g.AC3.1-2, P3g.AC4.1-4 (conditional) |

---

## Coverage Audit

**Total acceptance criteria:** 90

- Phase 3a: 15 (+1: P3a.AC4.6 daemon logs)
- Phase 3b: 14
- Phase 3c: 12
- Phase 3d: 15
- Phase 3e: 19 (+1: P3e.AC3.5 TaskBlocked mapping, +5: P3e.AC4.1-5 deferred emission)
- Phase 3f: 10
- Phase 3g: 10

**Automated test coverage:** 63 criteria (70%)

**Human verification only:** 15 criteria (17%) — includes P3a.AC4.6

**Build-time verification:** 6 criteria (7%)

**Deferred:** 6 criteria (7%) — P3e.AC2.5 (WebSocket heartbeat interval), P3e.AC4.1-5 (event emission for 5 WsEvent types)

All 90 acceptance criteria are mapped to either an automated test, a documented human verification procedure, a build-time check, or identified as deferred.

---

## New Dev Dependencies

```toml
[dev-dependencies]
tokio-tungstenite = "0.26"   # WebSocket client for daemon_ws_test
# tempfile already exists
```

---

## Cross-Phase Dependencies

| Criterion | Defined In | Tested In | Rationale |
|-----------|------------|-----------|-----------|
| P3b.AC5.2 | Phase 3b | Phase 3g (final wiring) | Full daemon start requires all modules to be wired |
| P3g.AC4.1-4 | Phase 3g | Conditional on `bundle-ui` | Web UI from Phase 4 must exist for embedded asset tests |
| P3e.AC2.5 | Phase 3e | Deferred | Heartbeat tuning is operational, not functional |
| P3e.AC4.1-5 | Phase 3e | Deferred | Event emission for `NodeStatusChanged`, `EdgeCreated`, `SessionEnded`, `ToolExecution`, `OrchestratorStateChanged` requires hooks in GraphStore, AgentRuntime, and Orchestrator that will be wired during daemon-orchestrator integration |

---

## Prerequisite Changes to Existing Code

The following changes to existing code are needed before Phase 3 implementation. Each is assigned to the phase that first requires it.

### Phase 3a prerequisites
1. **`libc` dependency**: Add `[target.'cfg(unix)'.dependencies] libc = "0.2"` for PID process checking.

### Phase 3b prerequisites
2. **`ProjectStore` needs `Clone`**: Already done — `ProjectStore` derives `Clone` in `src/project.rs`.
3. **`ProjectStore::get_by_id()` method**: Add a method to look up a project by its ID (the URL parameter `:id` may receive either a name or an ID). Query: `SELECT * FROM projects WHERE id = ?1`.

### Phase 3c prerequisites
4. **`NodeQuery` needs `Default`**: Add `#[derive(Default)]` to `NodeQuery` in `src/graph/store.rs`. All fields are `Option<T>`, so Default produces a query with no filters.

### Phase 3d prerequisites
5. **`ImportResult`, `DiffResult`, and `ImportConflict` need `Serialize`**: Add `#[derive(Serialize)]` to all three types in `src/graph/interchange.rs`.
6. **Refactor interchange/export functions to accept `&dyn GraphStore`**: `export_goal`, `import_goal`, `diff_goal` in `src/graph/interchange.rs` and `export_adrs` in `src/graph/export.rs` currently take `&SqliteGraphStore`. They must be changed to `&dyn GraphStore` because `AppState` holds `Arc<dyn GraphStore>`. These functions only use `GraphStore` trait methods, so this is a type-signature-only change — except `import_goal` which uses a `SqliteGraphStore`-specific helper (`import_nodes_and_edges`) that needs to be promoted to the trait or reimplemented using `create_node`/`add_edge`.

### Already done (verified)
7. **`SessionStore::get_session(id)` method**: Already exists in `src/graph/session.rs`.
8. **`SessionStore::list_sessions(goal_id)` method**: Already exists.
