# Rustagent V2 Phase 1a: Database + Projects

**Goal:** Set up the central SQLite database with WAL mode, schema migrations, and project registration CRUD with CLI.

**Architecture:** Single SQLite database at XDG data dir (`~/.local/share/rustagent/rustagent.db`). All access through one `tokio_rusqlite::Connection` (internally Arc-wrapped, Clone-cheap). WAL mode + `BEGIN IMMEDIATE` for all write transactions. Hand-rolled sequential migrations tracked via `schema_version` table.

**Tech Stack:** Rust (edition 2024), rusqlite 0.32 (bundled), tokio-rusqlite 0.6, clap 4.5 (derive), serde/serde_json, chrono, uuid, dirs, anyhow, tokio

**Scope:** Phase 1 of 4 from the v2 architecture (Phase 1a: Database + Projects)

**Codebase verified:** 2026-02-07

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P1a.AC1: Database initialization
- **P1a.AC1.1 Success:** Database file created at `~/.local/share/rustagent/rustagent.db` on first startup
- **P1a.AC1.2 Success:** WAL mode enabled, `foreign_keys = ON`, `busy_timeout = 5000`
- **P1a.AC1.3 Success:** All tables created: `schema_version`, `projects`, `nodes`, `edges`, `sessions`, `nodes_fts` (virtual), `worker_conversations`, plus FTS sync triggers and indexes
- **P1a.AC1.4 Success:** `schema_version` table contains version 1 after fresh init

### P1a.AC2: Schema versioning and migrations
- **P1a.AC2.1 Success:** Fresh database: full schema created, version set to 1
- **P1a.AC2.2 Success:** Database at current version: no migration runs, proceeds normally
- **P1a.AC2.3 Failure:** Database newer than binary: returns clear error "your database was created by a newer version of rustagent, please upgrade"

### P1a.AC3: Project registration
- **P1a.AC3.1 Success:** `rustagent project add <name> <path>` registers a project with auto-generated ID (`ra-` + 4 hex chars)
- **P1a.AC3.2 Success:** `rustagent project list` shows all registered projects
- **P1a.AC3.3 Success:** `rustagent project show <name>` returns project details
- **P1a.AC3.4 Success:** `rustagent project remove <name>` deletes a project record
- **P1a.AC3.5 Failure:** Adding a project with a duplicate name returns error
- **P1a.AC3.6 Success:** When no `--project` flag, CLI resolves project from current working directory by matching registered project paths

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Add new dependencies to Cargo.toml

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/Cargo.toml`

**Implementation:**

Add the following to `[dependencies]` (after the existing entries):

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
tokio-rusqlite = "0.6"
```

Remove the TUI dependencies that are no longer needed (v2 replaces TUI with web UI):

```toml
# REMOVE these three lines:
ratatui = "0.29"
crossterm = "0.28"
tui-textarea = { version = "0.7", default-features = false, features = ["crossterm"] }
```

**Verification:**

Run: `cargo check`
Expected: Compiles without errors (TUI removal will cause compile errors in src/tui/ and main.rs — that's expected and addressed in Task 2)

**Commit:** `chore: add rusqlite and tokio-rusqlite, remove TUI deps`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Remove TUI module and update main.rs/lib.rs

**Files:**
- Delete: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tui/` (entire directory)
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — remove `pub mod tui;`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — remove `Tui` variant from `Commands` enum, remove the `Commands::Tui` match arm, remove the `unwrap_or(Commands::Tui)` default (replace with showing help when no subcommand)

**Implementation:**

In `lib.rs`, remove the `pub mod tui;` line. Keep all other module declarations.

In `main.rs`:
- Remove the `Tui` variant from `Commands` enum
- Change `cli.command.unwrap_or(Commands::Tui)` to handle `None` by printing help and exiting
- Remove the entire `Commands::Tui` match arm and its `use rustagent::tui` import

The v1 `Init`, `Plan`, and `Run` commands stay for now — they'll be replaced incrementally as v2 modules come online.

**Verification:**

Run: `cargo check`
Expected: Compiles cleanly. No references to ratatui, crossterm, or tui remain.

Run: `cargo test`
Expected: All existing tests still pass (TUI had no tests).

**Commit:** `refactor: remove TUI module (replaced by web UI in v2)`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Create database module with initialization and migrations

**Verifies:** P1a.AC1.1, P1a.AC1.2, P1a.AC1.3, P1a.AC1.4, P1a.AC2.1, P1a.AC2.2, P1a.AC2.3

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/db/mod.rs`
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/db/migrations.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod db;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/db_test.rs` (integration)

**Implementation:**

`src/db/mod.rs`:
- `Database` struct wrapping `tokio_rusqlite::Connection`
- `Database::open(path: &Path) -> Result<Self>` — opens connection, calls `init_pragmas` then `run_migrations`
- `Database::open_in_memory() -> Result<Self>` — for testing
- `Database::connection(&self) -> &tokio_rusqlite::Connection` — accessor
- Private `init_pragmas(conn: &rusqlite::Connection)` — sets WAL, foreign_keys, busy_timeout, wal_autocheckpoint
- The `Database` should implement `Clone` (delegates to inner `Connection::clone()`)

`src/db/migrations.rs`:
- `const CURRENT_VERSION: u32 = 1;`
- `pub fn run_migrations(conn: &rusqlite::Connection) -> Result<()>`:
  1. Check if `schema_version` table exists (query `sqlite_master`)
  2. If not: fresh DB — run `create_schema_v1(conn)`, insert version 1
  3. If exists: read version. If == CURRENT_VERSION, return Ok. If > CURRENT_VERSION, return error. If < CURRENT_VERSION, run sequential migrations.
- `fn create_schema_v1(conn: &rusqlite::Connection) -> Result<()>` — all CREATE TABLE/INDEX/TRIGGER/VIRTUAL TABLE statements from the architecture doc's Database Schema section

The full schema includes: `schema_version`, `projects`, `nodes` (with 3 indexes), `edges` (with 3 indexes), `sessions`, `nodes_fts` (FTS5 virtual table with content sync), FTS sync triggers (nodes_ai, nodes_ad, nodes_au), `worker_conversations`.

**Testing:**

Tests must verify each AC listed above:
- P1a.AC1.1: `Database::open` creates file at specified path
- P1a.AC1.2: After open, query `PRAGMA journal_mode` returns "wal", `PRAGMA foreign_keys` returns 1
- P1a.AC1.3: After open, all tables exist in `sqlite_master` (projects, nodes, edges, sessions, nodes_fts, worker_conversations, schema_version)
- P1a.AC1.4: After fresh init, `schema_version` contains version 1
- P1a.AC2.1: `open_in_memory()` creates full schema and sets version 1
- P1a.AC2.2: Opening an already-initialized DB does not error and version remains 1
- P1a.AC2.3: Manually set version to 999, reopen — returns error containing "newer version"

Use `Database::open_in_memory()` for most tests. Use `tempfile::TempDir` for the file creation test.

Follow project testing patterns: integration tests in `tests/db_test.rs`, `#[tokio::test]` for async, `assert_eq!`/`assert!` for assertions.

**Verification:**
Run: `cargo test db_test`
Expected: All tests pass

**Commit:** `feat(db): database initialization with WAL mode, schema v1, and migration framework`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->

<!-- START_SUBCOMPONENT_B (tasks 4-5) -->

<!-- START_TASK_4 -->
### Task 4: Create Project type and store

**Verifies:** P1a.AC3.1, P1a.AC3.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/project.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod project;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/project_test.rs` (integration)

**Implementation:**

`src/project.rs`:

```rust
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub registered_at: DateTime<Utc>,
    pub config_overrides: Option<String>,  // JSON string, stored as TEXT in DB
    pub metadata: String,                  // JSON string, stored as TEXT in DB, default "{}"
}
```

**Architecture deviation:** The architecture shows `config_overrides: Option<ProjectConfig>` and `metadata: HashMap<String, String>`. This implementation stores both as JSON strings in the database (matching the SQLite TEXT columns). The typed structs can be deserialized on demand when accessed by application code. This avoids a deserialization step on every DB read and matches the DB schema directly.

`ProjectStore` struct wrapping `Database`:
- `new(db: Database) -> Self`
- `async fn add(&self, name: &str, path: &Path) -> Result<Project>` — generates ID (`ra-` + 4 hex chars from uuid v4), inserts with `BEGIN IMMEDIATE`, returns the created `Project`. Fails if name already exists (UNIQUE constraint).
- `async fn list(&self) -> Result<Vec<Project>>` — returns all projects ordered by name
- `async fn get_by_name(&self, name: &str) -> Result<Option<Project>>` — lookup by name
- `async fn get_by_path(&self, path: &Path) -> Result<Option<Project>>` — lookup by path (for cwd resolution). Canonicalize both stored path and query path before comparison.
- `async fn remove(&self, name: &str) -> Result<bool>` — delete by name, returns true if deleted

ID generation: `format!("ra-{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..4])`

All write operations use `BEGIN IMMEDIATE` via `conn.call(|conn| { let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?; ... })`.

**Testing:**

Tests must verify:
- P1a.AC3.1: `add("my-api", "/tmp/test")` creates project with `ra-` prefixed 4-char hex ID, correct name and path
- P1a.AC3.5: Adding two projects with the same name returns an error

Use `Database::open_in_memory()`. Follow project patterns: `#[tokio::test]`, `tests/project_test.rs`.

**Verification:**
Run: `cargo test project_test`
Expected: All tests pass

**Commit:** `feat(project): Project type and ProjectStore with CRUD operations`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Project store — list, show, remove, resolve from cwd

**Verifies:** P1a.AC3.2, P1a.AC3.3, P1a.AC3.4, P1a.AC3.6

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/project.rs` (if not already covered in Task 4)
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/project_test.rs` (add more tests)

**Implementation:**

This task adds tests for the remaining ProjectStore methods implemented in Task 4. If any methods were left as stubs, implement them now.

**Testing:**

Tests must verify:
- P1a.AC3.2: After adding 3 projects, `list()` returns all 3 ordered by name
- P1a.AC3.3: After adding a project, `get_by_name("my-api")` returns the project with correct details
- P1a.AC3.4: After adding then removing a project, `get_by_name` returns None
- P1a.AC3.6: After adding a project with path `/tmp/test-proj`, `get_by_path("/tmp/test-proj")` returns it

**Verification:**
Run: `cargo test project_test`
Expected: All tests pass

**Commit:** `test(project): complete project store test coverage`

<!-- END_TASK_5 -->
<!-- END_SUBCOMPONENT_B -->

<!-- START_SUBCOMPONENT_C (tasks 6-7) -->

<!-- START_TASK_6 -->
### Task 6: Wire up `project` CLI subcommand

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add `Project` subcommand with nested `add`/`list`/`show`/`remove`

**Implementation:**

Add a `Project` variant to `Commands` with nested subcommands:

```rust
/// Manage projects
Project {
    #[command(subcommand)]
    action: ProjectAction,
},
```

```rust
#[derive(Subcommand)]
enum ProjectAction {
    /// Register a project
    Add {
        /// Friendly name for the project
        name: String,
        /// Path to the project directory
        path: String,
    },
    /// List all registered projects
    List,
    /// Show project details
    Show {
        /// Project name
        name: String,
    },
    /// Remove a registered project
    Remove {
        /// Project name
        name: String,
    },
}
```

In the `main()` match:
- `ProjectAction::Add` — open database (see helper below), create `ProjectStore`, call `add()`, print the created project
- `ProjectAction::List` — open database, list, print formatted table
- `ProjectAction::Show` — open database, get_by_name, print details (or "not found")
- `ProjectAction::Remove` — open database, remove, print confirmation

Add a helper function to get the database path and open it:

```rust
fn db_path() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine XDG data directory"))?;
    let db_dir = data_dir.join("rustagent");
    std::fs::create_dir_all(&db_dir)?;
    Ok(db_dir.join("rustagent.db"))
}
```

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- project add test-proj .`
Expected: Prints something like "Registered project 'test-proj' (ra-a3f8) at /Users/david.hagerty/code/personal/rustagent/new-directions"

Run: `cargo run -- project list`
Expected: Shows the registered project

Run: `cargo run -- project show test-proj`
Expected: Shows project details

Run: `cargo run -- project remove test-proj`
Expected: Prints confirmation

**Commit:** `feat(cli): add project subcommand (add/list/show/remove)`

<!-- END_TASK_6 -->

<!-- START_TASK_7 -->
### Task 7: Add `--project` flag and cwd resolution to CLI

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add global `--project` flag to `Cli` struct

**Implementation:**

Add a `--project` flag to the top-level `Cli` struct:

```rust
#[derive(Parser)]
#[command(name = "rustagent")]
struct Cli {
    /// Project name (if omitted, resolves from current directory)
    #[arg(long, global = true)]
    project: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}
```

Add a helper function `resolve_project` that:
1. If `--project <name>` was given, looks up by name
2. If not, gets the current working directory and looks up by path
3. Returns `Option<Project>` (None is valid — some commands don't need a project)

This resolver is not used by the `project` subcommand itself (which takes explicit name args), but will be used by later commands like `run`, `tasks`, `status`. For now, just define the function — it gets exercised when those commands are wired up in Phase 1b.

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- --help`
Expected: Shows `--project` in global options

**Commit:** `feat(cli): add --project global flag with cwd resolution helper`

<!-- END_TASK_7 -->
<!-- END_SUBCOMPONENT_C -->
