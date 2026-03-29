# Rustagent V2 Phase 1c: Sessions + Export + Interchange

**Goal:** Implement session management with deterministic handoff notes, ADR export to markdown, TOML graph import/export/diff, node decay for context injection, and associated CLI commands.

**Architecture:** Sessions are temporal records (not graph nodes) tracking work periods per goal. Handoff notes are template-generated from graph state queries — no LLM call. TOML interchange uses one file per goal in `.rustagent/graph/`, with deterministic key ordering (BTreeMap) for git-friendly diffs. Node decay compacts old nodes for context injection based on configurable age thresholds.

**Tech Stack:** Rust (edition 2024), rusqlite 0.32 (bundled), tokio-rusqlite 0.6, toml 0.8, blake3 1.x, chrono, serde/serde_json, anyhow

**Scope:** Phase 3 of 4 from the v2 architecture (Phase 1c: Sessions + Export + Interchange)

**Codebase verified:** 2026-02-07

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

**Depends on:** Phase 1a (database), Phase 1b (graph store, node types)

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P1c.AC1: Session management
- **P1c.AC1.1 Success:** `create_session(goal_id)` creates a session record with start time and goal reference
- **P1c.AC1.2 Success:** `end_session(session_id)` generates deterministic handoff notes from graph state and stores them
- **P1c.AC1.3 Success:** Handoff notes contain Done, Remaining, Blocked, and Decisions Made sections populated from graph queries
- **P1c.AC1.4 Success:** `get_latest_session(goal_id)` returns the most recent session with handoff notes

### P1c.AC2: ADR export
- **P1c.AC2.1 Success:** `export_adrs(project_id, output_dir)` generates numbered markdown files (001-xxx.md) from Decision nodes
- **P1c.AC2.2 Success:** Each ADR contains Status, Context, Options Considered (with Chosen/Rejected labels, pros/cons), Outcome, and Related Tasks sections

### P1c.AC3: TOML graph interchange
- **P1c.AC3.1 Success:** `export_goal(goal_id)` produces a TOML file matching the format in the architecture (meta, nodes, edges sections)
- **P1c.AC3.2 Success:** Output is deterministic — re-exporting unchanged state produces byte-identical output (sorted keys, omitted null fields)
- **P1c.AC3.3 Success:** `import_goal(toml_content, strategy)` imports nodes and edges with merge/theirs/ours conflict strategies
- **P1c.AC3.4 Success:** Round-trip: export -> import -> export produces identical files
- **P1c.AC3.5 Success:** `diff_goal(toml_content, goal_id)` shows added/changed/unchanged entities
- **P1c.AC3.6 Success:** Cross-goal edge references to nonexistent nodes are skipped with a clear error message

### P1c.AC4: Node decay
- **P1c.AC4.1 Success:** Nodes < 7 days old: full detail (description, criteria, outcomes)
- **P1c.AC4.2 Success:** Nodes 7-30 days old: summary only (title, status, key outcome)
- **P1c.AC4.3 Success:** Nodes > 30 days old: minimal (title, status)
- **P1c.AC4.4 Success:** Thresholds are configurable

### P1c.AC5: CLI commands
- **P1c.AC5.1 Success:** `rustagent sessions` lists sessions for current goal
- **P1c.AC5.2 Success:** `rustagent sessions latest` shows most recent handoff notes
- **P1c.AC5.3 Success:** `rustagent decisions export` writes ADR markdown files
- **P1c.AC5.4 Success:** `rustagent graph export` / `rustagent graph import` / `rustagent graph diff` work

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Session management and handoff notes

**Verifies:** P1c.AC1.1, P1c.AC1.2, P1c.AC1.3, P1c.AC1.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/session.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs` — add `pub mod session;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/session_test.rs`

**Implementation:**

`src/graph/session.rs`:

```rust
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub goal_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub handoff_notes: Option<String>,
    pub agent_ids: Vec<String>,  // JSON serialized
    pub summary: Option<String>,
}
```

`SessionStore` wrapping `Database`:
- `async fn create_session(&self, project_id: &str, goal_id: &str) -> Result<Session>` — generates ID, inserts with `BEGIN IMMEDIATE`
- `async fn end_session(&self, session_id: &str, graph_store: &SqliteGraphStore) -> Result<()>` — calls `generate_handoff_notes`, updates ended_at and handoff_notes
- `async fn get_latest_session(&self, goal_id: &str) -> Result<Option<Session>>` — SELECT ORDER BY started_at DESC LIMIT 1
- `async fn list_sessions(&self, goal_id: &str) -> Result<Vec<Session>>`

`fn generate_handoff_notes(conn: &rusqlite::Connection, goal_id: &str) -> rusqlite::Result<String>`:
Template-based, queries graph state:

```
## Done
{for each node under goal with status Completed or Decided}
- {id}: {title} ({status})

## Remaining
{for each node under goal with status Ready, Pending, or InProgress}
- {id}: {title} ({status}{, blocked by X if blocked})

## Blocked
{for each node under goal with status Blocked}
- {id}: {title} — {blocked_reason}

## Decisions Made
{for each Decision node under goal with status Decided}
- {id}: {title} → {chosen option title} ({rationale from Chosen edge label})
```

This runs within the `end_session` call's `BEGIN IMMEDIATE` transaction using direct SQL queries on the raw `rusqlite::Connection` (not the async `GraphStore` methods — it's inside the `.call()` closure where only synchronous rusqlite is available). The queries duplicate some of what `SqliteGraphStore` does but operate directly on the connection for transactional consistency.

**Testing:**

Tests must verify:
- P1c.AC1.1: `create_session` returns a session with valid ID and start time
- P1c.AC1.2: After creating tasks (some completed, some pending) and a decided decision under a goal, `end_session` produces handoff notes
- P1c.AC1.3: Handoff notes contain all 4 sections with correct content from the graph
- P1c.AC1.4: `get_latest_session` returns the most recent of 2 sessions

Set up test data by creating nodes and edges via `SqliteGraphStore` before calling session functions.

**Verification:**
Run: `cargo test session_test`
Expected: All tests pass

**Commit:** `feat(graph): session management with deterministic handoff notes`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: ADR export

**Verifies:** P1c.AC2.1, P1c.AC2.2

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/export.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs` — add `pub mod export;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/adr_export_test.rs`

**Implementation:**

`src/graph/export.rs`:

`pub async fn export_adrs(graph_store: &SqliteGraphStore, project_id: &str, output_dir: &Path) -> Result<Vec<PathBuf>>`:
1. Query all Decision nodes for the project (any status — Active, Decided, Superseded)
2. For each Decision, gather connected Options (via LeadsTo edges), their Chosen/Rejected status (via Chosen/Rejected edges), and any Outcome nodes (via LeadsTo from related tasks)
3. Number sequentially (001, 002, ...) ordered by created_at
4. Generate markdown per the architecture format (lines 583-596 of design doc)
5. Write to `output_dir/001-<slugified-title>.md`
6. Return list of written file paths

Title slugification: lowercase, replace spaces with hyphens, strip non-alphanumeric except hyphens.

**Testing:**

Tests must verify:
- P1c.AC2.1: Create 2 decisions in a project. Export to a tempdir. Two files created: `001-*.md` and `002-*.md`.
- P1c.AC2.2: Open the exported file. Verify it contains Status, Context, Options Considered (with CHOSEN/REJECTED labels and pros/cons), and Related Tasks sections.

**Verification:**
Run: `cargo test adr_export_test`
Expected: All tests pass

**Commit:** `feat(graph): ADR export to markdown`

<!-- END_TASK_2 -->
<!-- END_SUBCOMPONENT_A -->

<!-- START_SUBCOMPONENT_B (tasks 3-5) -->

<!-- START_TASK_3 -->
### Task 3: TOML interchange — export

**Verifies:** P1c.AC3.1, P1c.AC3.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/Cargo.toml` — add `blake3` dependency
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/interchange.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs` — add `pub mod interchange;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/interchange_test.rs`

**Prerequisite:** Add `blake3 = "1"` to `[dependencies]` in Cargo.toml. Run `cargo check` to verify.

**Implementation:**

`src/graph/interchange.rs`:

Define serde types for the TOML format (separate from the DB types to control serialization):

```rust
#[derive(Serialize, Deserialize)]
struct GoalFile {
    meta: Meta,
    nodes: BTreeMap<String, TomlNode>,  // BTreeMap for sorted keys
    edges: BTreeMap<String, TomlEdge>,
}

#[derive(Serialize, Deserialize)]
struct Meta {
    version: u32,
    goal_id: String,
    project: String,
    exported_at: String,
    content_hash: String,
}
```

`TomlNode` and `TomlEdge` are simplified serde structs that map to the TOML format shown in the architecture (lines 726-814). Use `#[serde(skip_serializing_if = "Option::is_none")]` to omit null fields.

`pub async fn export_goal(graph_store: &SqliteGraphStore, goal_id: &str, project_name: &str) -> Result<String>`:
1. Get subtree of goal node (all descendants)
2. Get all edges where either from_node or to_node is in the subtree
3. Convert to `GoalFile` with BTreeMap for deterministic ordering
4. Compute content hash: blake3 hash of serialized nodes+edges (excluding meta)
5. Serialize with `toml::to_string_pretty()`
6. Return the TOML string

Use BTreeMap (not HashMap) for the nodes and edges maps — toml crate's Map type is BTreeMap by default, which gives sorted key order.

**Testing:**

Tests must verify:
- P1c.AC3.1: Create a goal with tasks, decisions, options, edges. Export. Parse the TOML string. Verify meta, nodes, and edges sections exist with correct data.
- P1c.AC3.2: Export twice without changes. Both strings are byte-identical.

**Verification:**
Run: `cargo test interchange_test`
Expected: All tests pass

**Commit:** `feat(graph): TOML export for goal files`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: TOML interchange — import with conflict strategies

**Verifies:** P1c.AC3.3, P1c.AC3.4, P1c.AC3.6

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/interchange.rs`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/interchange_test.rs` (add more tests)

**Implementation:**

```rust
pub enum ImportStrategy { Merge, Theirs, Ours }

pub struct ImportResult {
    pub added_nodes: usize,
    pub added_edges: usize,
    pub conflicts: Vec<ImportConflict>,
    pub skipped_edges: Vec<String>,  // Cross-goal refs to nonexistent nodes
    pub unchanged: usize,
}

pub struct ImportConflict {
    pub node_id: String,
    pub field: String,
    pub db_value: String,
    pub file_value: String,
}
```

`pub async fn import_goal(graph_store: &SqliteGraphStore, toml_content: &str, strategy: ImportStrategy) -> Result<ImportResult>`:
1. Parse TOML into `GoalFile`
2. For each node: check if it exists in DB
   - New node: insert
   - Existing, unchanged: skip
   - Existing, changed: apply strategy (Merge=flag conflict, Theirs=file wins, Ours=skip)
3. For each edge: check if both from_node and to_node exist in DB
   - Both exist: insert edge (idempotent — skip if edge already exists)
   - Target node missing: add to `skipped_edges` with clear message
4. Return `ImportResult`

All writes in one `BEGIN IMMEDIATE` transaction.

**Testing:**

Tests must verify:
- P1c.AC3.3: Export a goal, modify a node in the TOML string, import with `Theirs` strategy. Verify DB has the modified value.
- P1c.AC3.4: Export, import into a fresh DB, export again. Both TOML strings are identical.
- P1c.AC3.6: Create an edge in TOML referencing a nonexistent node ID. Import. Edge is skipped and appears in `skipped_edges` with a descriptive message.

**Verification:**
Run: `cargo test interchange_test`
Expected: All tests pass

**Commit:** `feat(graph): TOML import with merge/theirs/ours conflict resolution`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: TOML interchange — diff

**Verifies:** P1c.AC3.5

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/interchange.rs`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/interchange_test.rs` (add diff test)

**Implementation:**

```rust
pub struct DiffResult {
    pub added_nodes: Vec<String>,
    pub changed_nodes: Vec<(String, Vec<String>)>,  // (id, changed_fields)
    pub removed_nodes: Vec<String>,  // in DB but not in file
    pub added_edges: Vec<String>,
    pub removed_edges: Vec<String>,
    pub unchanged_nodes: usize,
    pub unchanged_edges: usize,
}
```

`pub async fn diff_goal(graph_store: &SqliteGraphStore, toml_content: &str) -> Result<DiffResult>`:
1. Parse TOML
2. Compare each node/edge against DB state
3. Report differences without making any changes

**Testing:**

- P1c.AC3.5: Create a goal in DB. Export to TOML. Add a node and change another in the TOML string. Diff. Verify `added_nodes` has 1 entry, `changed_nodes` has 1 entry with the changed fields listed.

**Verification:**
Run: `cargo test interchange_test`
Expected: All tests pass

**Commit:** `feat(graph): TOML diff against DB state`

<!-- END_TASK_5 -->
<!-- END_SUBCOMPONENT_B -->

<!-- START_SUBCOMPONENT_C (tasks 6-7) -->

<!-- START_TASK_6 -->
### Task 6: Node decay for context injection

**Verifies:** P1c.AC4.1, P1c.AC4.2, P1c.AC4.3, P1c.AC4.4

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/decay.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs` — add `pub mod decay;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/decay_test.rs`

**Implementation:**

`src/graph/decay.rs`:

```rust
pub struct DecayConfig {
    pub recent_days: i64,  // Default: 7
    pub older_days: i64,   // Default: 30
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self { recent_days: 7, older_days: 30 }
    }
}

pub enum DecayLevel { Full, Summary, Minimal }

pub struct DecayedNode {
    pub id: String,
    pub title: String,
    pub status: NodeStatus,
    pub detail: DecayDetail,
}

pub enum DecayDetail {
    Full { description: String, metadata: HashMap<String, String> },
    Summary { key_outcome: Option<String> },
    Minimal,
}
```

`pub fn decay_node(node: &GraphNode, now: DateTime<Utc>, config: &DecayConfig) -> DecayedNode`:
- Calculate age from `completed_at` (or `created_at` if not completed)
- If age < `recent_days`: Full detail
- If age < `older_days`: Summary — title, status, extract key outcome from metadata if present
- Else: Minimal — title and status only

`pub fn decay_nodes(nodes: &[GraphNode], now: DateTime<Utc>, config: &DecayConfig) -> Vec<DecayedNode>`:
- Apply `decay_node` to each

**Testing:**

Tests must verify:
- P1c.AC4.1: Node completed 2 days ago → DecayDetail::Full with description and metadata
- P1c.AC4.2: Node completed 15 days ago → DecayDetail::Summary with title and status
- P1c.AC4.3: Node completed 45 days ago → DecayDetail::Minimal with title and status only
- P1c.AC4.4: Custom config with `recent_days: 3, older_days: 10`. Node completed 5 days ago → Summary (not Full).

**Verification:**
Run: `cargo test decay_test`
Expected: All tests pass

**Commit:** `feat(graph): node decay for context injection with configurable thresholds`

<!-- END_TASK_6 -->

<!-- START_TASK_7 -->
### Task 7: Wire up session, export, and interchange CLI commands

**Verifies:** P1c.AC5.1, P1c.AC5.2, P1c.AC5.3, P1c.AC5.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add `Sessions`, `Graph` subcommands, extend `Decisions` with `Export` action

**Implementation:**

Add CLI variants:

```rust
/// View sessions and handoff notes
Sessions {
    #[command(subcommand)]
    action: Option<SessionAction>,
},
/// Import/export graph data
Graph {
    #[command(subcommand)]
    action: GraphAction,
},
```

```rust
#[derive(Subcommand)]
enum SessionAction {
    /// List sessions for current goal
    List { #[arg(long)] goal: Option<String> },
    /// Show most recent handoff notes
    Latest { #[arg(long)] goal: Option<String> },
}

#[derive(Subcommand)]
enum GraphAction {
    /// Export goals to TOML files
    Export {
        #[arg(long)] goal: Option<String>,
        #[arg(long)] output: Option<String>,
    },
    /// Import TOML files
    Import {
        path: String,
        #[arg(long)] dry_run: bool,
        #[arg(long)] theirs: bool,
        #[arg(long)] ours: bool,
    },
    /// Diff TOML file against DB
    Diff { path: String },
}
```

Add `Export` variant to `DecisionAction`:
```rust
/// Export decisions as ADR markdown files
Export {
    #[arg(long)] output: Option<String>,
},
```

Each command resolves project, opens DB, creates stores, calls functions, prints results.

**Note:** Session TOML export (`.rustagent/sessions/*.toml` files as described in the architecture) is deferred to a later phase. This phase covers session DB records and handoff notes generation, but not the file-based session export format.

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- sessions --help`
Expected: Shows list/latest subcommands

Run: `cargo run -- graph --help`
Expected: Shows export/import/diff subcommands

**Commit:** `feat(cli): sessions, graph interchange, and ADR export commands`

<!-- END_TASK_7 -->
<!-- END_SUBCOMPONENT_C -->
