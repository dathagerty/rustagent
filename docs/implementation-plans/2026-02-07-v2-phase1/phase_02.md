# Rustagent V2 Phase 1b: Graph Model + Node Lifecycle

**Goal:** Implement the unified work graph — types, SQLite store, query builders, dependency resolution, FTS5 search, graph tools for agents, and CLI commands.

**Architecture:** All entities (goals, tasks, decisions, options, outcomes, observations, revisits) are nodes in one DAG. Relationships are edges. Task surfacing (`ready`, `next`) is derived from dependency resolution over `DependsOn` edges. FTS5 provides full-text search. Atomic task claiming via conditional UPDATE. All writes through single `tokio_rusqlite::Connection` with `BEGIN IMMEDIATE`.

**Tech Stack:** Rust (edition 2024), rusqlite 0.32 (bundled), tokio-rusqlite 0.6, async-trait, serde/serde_json, chrono, uuid, anyhow

**Scope:** Phase 2 of 4 from the v2 architecture (Phase 1b: Graph Model + Node Lifecycle)

**Codebase verified:** 2026-02-07

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

**Depends on:** Phase 1a (database module, project store)

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P1b.AC1: Graph node types and data model
- **P1b.AC1.1 Success:** `GraphNode` struct with all fields from architecture (id, project_id, node_type, title, description, status, priority, assigned_to, created_by, labels, timestamps, blocked_reason, metadata)
- **P1b.AC1.2 Success:** 7 `NodeType` variants: Goal, Task, Decision, Option, Outcome, Observation, Revisit
- **P1b.AC1.3 Success:** `NodeStatus` enum with all variants, valid status transitions per node type enforced
- **P1b.AC1.4 Success:** `GraphEdge` struct with id, edge_type, from_node, to_node, label, created_at
- **P1b.AC1.5 Success:** 7 `EdgeType` variants: Contains, DependsOn, LeadsTo, Chosen, Rejected, Supersedes, Informs

### P1b.AC2: Hierarchical ID generation
- **P1b.AC2.1 Success:** Goal IDs: `ra-` + 4 hex chars from UUID v4
- **P1b.AC2.2 Success:** Child IDs: parent_id + `.N` where N is sequential counter from parent's `next_child_seq` metadata
- **P1b.AC2.3 Success:** Full dotted path is the primary key (e.g., `ra-a3f8.1.3`)
- **P1b.AC2.4 Success:** Edge IDs: `e-` + 8 hex chars from UUID v4

### P1b.AC3: GraphStore CRUD
- **P1b.AC3.1 Success:** `create_node` inserts node with all fields, returns Ok
- **P1b.AC3.2 Success:** `get_node(id)` returns the node or None
- **P1b.AC3.3 Success:** `update_node` modifies status and/or metadata
- **P1b.AC3.4 Success:** `add_edge` inserts edge; `get_edges` returns edges for a node
- **P1b.AC3.5 Success:** `get_children(node_id)` returns child nodes via Contains edges
- **P1b.AC3.6 Success:** `get_subtree(node_id)` returns all descendant nodes recursively

### P1b.AC4: Dependency resolution and task surfacing
- **P1b.AC4.1 Success:** Task moves from Pending to Ready when all DependsOn targets are Completed
- **P1b.AC4.2 Success:** `get_ready_tasks(goal_id)` returns only tasks in Ready status with all deps satisfied
- **P1b.AC4.3 Success:** `get_next_task(goal_id)` returns highest-priority Ready task, breaking ties by downstream unblock count

### P1b.AC5: Atomic task claiming
- **P1b.AC5.1 Success:** `claim_task(node_id, agent_id)` sets status to Claimed and assigned_to atomically
- **P1b.AC5.2 Success:** If task is not Ready, claim returns false (another worker got there first)

### P1b.AC6: Full-text search
- **P1b.AC6.1 Success:** `search_nodes(query)` returns nodes matching title or description via FTS5
- **P1b.AC6.2 Success:** Search can filter by project_id and node_type

### P1b.AC7: Graph tools for agents
- **P1b.AC7.1 Success:** All low-level tools work: `create_node`, `update_node`, `add_edge`, `query_nodes`, `search_nodes`
- **P1b.AC7.2 Success:** All high-level tools work: `claim_task`, `log_decision`, `choose_option`, `record_outcome`, `record_observation`, `revisit`

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Graph types — NodeType, EdgeType, NodeStatus, Priority, GraphNode, GraphEdge

**Verifies:** P1b.AC1.1, P1b.AC1.2, P1b.AC1.3, P1b.AC1.4, P1b.AC1.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod graph;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_types_test.rs`

**Implementation:**

`src/graph/mod.rs` — define these types (all `#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]`):

- `NodeType` enum: Goal, Task, Decision, Option, Outcome, Observation, Revisit. Implement `Display` and `FromStr` for serialization to/from the lowercase string used in the DB (`"goal"`, `"task"`, etc.).

- `EdgeType` enum: Contains, DependsOn, LeadsTo, Chosen, Rejected, Supersedes, Informs. Same Display/FromStr pattern.

- `NodeStatus` enum: Pending, Active, Completed, Cancelled, Ready, Claimed, InProgress, Review, Blocked, Failed, Decided, Superseded, Abandoned, Chosen, Rejected. Same Display/FromStr.

- `Priority` enum: Critical, High, Medium, Low. Same Display/FromStr.

- `GraphNode` struct per architecture (line 453-469 of design doc). Fields:
  - `id: String`, `project_id: String`, `node_type: NodeType`, `title: String`, `description: String`, `status: NodeStatus`, `priority: Option<Priority>`, `assigned_to: Option<String>`, `created_by: Option<String>`, `labels: Vec<String>`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `blocked_reason: Option<String>`, `metadata: HashMap<String, String>`

- `GraphEdge` struct per architecture (line 393-401). Fields:
  - `id: String`, `edge_type: EdgeType`, `from_node: String`, `to_node: String`, `label: Option<String>`, `created_at: DateTime<Utc>`

- `fn valid_statuses(node_type: &NodeType) -> Vec<NodeStatus>` — returns the valid statuses for each node type per the architecture (lines 442-448).

- `fn validate_status(node_type: &NodeType, status: &NodeStatus) -> Result<()>` — returns error if status is not valid for the node type.

Declare sub-modules: `pub mod store;`, `pub mod dependency;`

**Note:** The architecture lists a separate `src/graph/query.rs` for query builders. In this implementation, `NodeQuery`, `EdgeDirection`, and `WorkGraph` are defined directly in `store.rs` alongside the `GraphStore` trait, avoiding premature file separation. The query builder functionality is part of the trait contract.

**Testing:**

Tests must verify:
- P1b.AC1.2: All 7 NodeType variants roundtrip through Display/FromStr
- P1b.AC1.3: `validate_status(Task, Ready)` is Ok; `validate_status(Goal, Ready)` is Err
- P1b.AC1.5: All 7 EdgeType variants roundtrip through Display/FromStr
- Serialization: GraphNode and GraphEdge serialize to/from JSON correctly

**Verification:**
Run: `cargo test graph_types_test`
Expected: All tests pass

**Commit:** `feat(graph): core types — NodeType, EdgeType, NodeStatus, GraphNode, GraphEdge`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: ID generation helpers

**Verifies:** P1b.AC2.1, P1b.AC2.2, P1b.AC2.3, P1b.AC2.4

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/mod.rs` — add ID generation functions
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_types_test.rs` — add ID tests

**Implementation:**

Add to `src/graph/mod.rs`:

- `pub fn generate_goal_id() -> String` — `format!("ra-{}", &uuid::Uuid::new_v4().simple().to_string()[..4])`
- `pub fn generate_child_id(parent_id: &str, seq: u32) -> String` — `format!("{}.{}", parent_id, seq)`
- `pub fn generate_edge_id() -> String` — `format!("e-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])`
- `pub fn parent_id(id: &str) -> Option<&str>` — extracts parent from hierarchical ID (e.g., `"ra-a3f8.1.3"` -> `Some("ra-a3f8.1")`)

**Testing:**

Tests must verify:
- P1b.AC2.1: `generate_goal_id()` starts with `"ra-"` and has 4 hex chars after prefix
- P1b.AC2.2: `generate_child_id("ra-a3f8", 1)` returns `"ra-a3f8.1"`; `generate_child_id("ra-a3f8.1", 3)` returns `"ra-a3f8.1.3"`
- P1b.AC2.3: Generated IDs are valid as primary keys (no special chars beyond `-` and `.`)
- P1b.AC2.4: `generate_edge_id()` starts with `"e-"` and has 8 hex chars
- `parent_id("ra-a3f8.1.3")` returns `Some("ra-a3f8.1")`; `parent_id("ra-a3f8")` returns `None`

**Verification:**
Run: `cargo test graph_types_test`
Expected: All tests pass

**Commit:** `feat(graph): hierarchical ID generation (goal, child, edge)`

<!-- END_TASK_2 -->
<!-- END_SUBCOMPONENT_A -->

<!-- START_SUBCOMPONENT_B (tasks 3-4) -->

<!-- START_TASK_3 -->
### Task 3: GraphStore trait definition

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/store.rs`

**Implementation:**

Define the `GraphStore` trait from the architecture (lines 1742-1778 of design doc). This is a trait definition only — no implementation yet.

**Architecture deviation:** The architecture's `update_node` takes `&GraphNode` (full struct replacement). This implementation uses a partial-update signature with optional fields instead, which avoids read-modify-write races — callers update only the fields they intend to change without needing to read the full node first.

```rust
#[async_trait]
pub trait GraphStore: Send + Sync {
    // Node CRUD
    async fn create_node(&self, node: &GraphNode) -> Result<()>;
    async fn update_node(&self, id: &str, status: Option<NodeStatus>, title: Option<&str>, description: Option<&str>, metadata: Option<&HashMap<String, String>>) -> Result<()>;
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>>;
    async fn query_nodes(&self, query: &NodeQuery) -> Result<Vec<GraphNode>>;

    // Task-specific
    async fn claim_task(&self, node_id: &str, agent_id: &str) -> Result<bool>;
    async fn get_ready_tasks(&self, goal_id: &str) -> Result<Vec<GraphNode>>;
    async fn get_next_task(&self, goal_id: &str) -> Result<Option<GraphNode>>;

    // Edge operations
    async fn add_edge(&self, edge: &GraphEdge) -> Result<()>;
    async fn remove_edge(&self, edge_id: &str) -> Result<()>;
    async fn get_edges(&self, node_id: &str, direction: EdgeDirection) -> Result<Vec<(GraphEdge, GraphNode)>>;

    // Graph queries
    async fn get_children(&self, node_id: &str) -> Result<Vec<(GraphNode, EdgeType)>>;
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<GraphNode>>;
    async fn get_active_decisions(&self, project_id: &str) -> Result<Vec<GraphNode>>; // Now mode
    async fn get_full_graph(&self, goal_id: &str) -> Result<WorkGraph>; // History mode
    async fn search_nodes(&self, query: &str, project_id: Option<&str>, node_type: Option<NodeType>, limit: usize) -> Result<Vec<GraphNode>>;

    // Child ID sequencing
    async fn next_child_seq(&self, parent_id: &str) -> Result<u32>;
}

pub enum EdgeDirection { Outgoing, Incoming, Both }

pub struct WorkGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub struct NodeQuery {
    pub node_type: Option<NodeType>,
    pub status: Option<NodeStatus>,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub query: Option<String>,
}
```

**Verification:**
Run: `cargo check`
Expected: Compiles (trait is unused for now, that's fine)

**Commit:** `feat(graph): GraphStore trait definition`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: SqliteGraphStore — node and edge CRUD

**Verifies:** P1b.AC3.1, P1b.AC3.2, P1b.AC3.3, P1b.AC3.4, P1b.AC3.5, P1b.AC3.6

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/store.rs` — add `SqliteGraphStore` implementing `GraphStore`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_store_test.rs`

**Implementation:**

`SqliteGraphStore` struct wrapping `Database`:

- `new(db: Database) -> Self`
- Implement all `GraphStore` trait methods using `db.connection().call(...)` with `BEGIN IMMEDIATE` for writes.

Key implementation details:

- **create_node**: INSERT into `nodes` table. Serialize `labels` as JSON array, `metadata` as JSON object. If the node has a parent (determined by `parent_id()` helper), call `next_child_seq` first to get the sequence number. Store `next_child_seq` in the parent's metadata atomically (read current seq, increment, update parent metadata, insert child — all in one `BEGIN IMMEDIATE` transaction).

- **update_node**: UPDATE with optional fields. Only set columns that are `Some`. Validate status against node_type before updating.

- **get_node**: SELECT by id. Deserialize labels from JSON array, metadata from JSON object.

- **add_edge**: INSERT into edges. Validate that both from_node and to_node exist.

- **get_edges**: SELECT edges + JOIN nodes based on direction (Outgoing: from_node = id; Incoming: to_node = id; Both: either).

- **get_children**: SELECT nodes joined via edges WHERE edge_type = 'contains' AND from_node = parent_id.

- **get_subtree**: Recursive CTE (`WITH RECURSIVE`) walking Contains edges downward from the given node.

- **get_active_decisions**: Query Decision nodes for the project where status is Active or Decided. "Now mode" — returns the current truth (active decisions only, no abandoned/superseded).

- **get_full_graph**: Get the full subtree of nodes under a goal, plus all edges involving those nodes. Returns a `WorkGraph` struct containing both nodes and edges. "History mode" — includes abandoned paths and superseded decisions.

- **next_child_seq**: Read parent node's `metadata.next_child_seq` (default 1 if absent), increment it, update parent metadata, return the old value. All in one `BEGIN IMMEDIATE` transaction.

Row-to-struct mapping: implement a helper function `fn row_to_node(row: &rusqlite::Row) -> rusqlite::Result<GraphNode>` that maps column indices to struct fields. Same for `row_to_edge`.

**Testing:**

Tests must verify each AC:
- P1b.AC3.1: Create a goal node, verify it's retrievable
- P1b.AC3.2: `get_node` returns None for nonexistent ID
- P1b.AC3.3: Create node as Pending, update to Active, verify status changed
- P1b.AC3.4: Create two nodes, add Contains edge, verify `get_edges(Outgoing)` returns it
- P1b.AC3.5: Create goal + 2 child tasks via Contains edges, verify `get_children` returns both
- P1b.AC3.6: Create goal -> task -> subtask chain, verify `get_subtree(goal_id)` returns all 3
- `get_active_decisions`: Create 2 decisions under a project — one Active, one Superseded. Verify `get_active_decisions` returns only the Active one.
- `get_full_graph`: Create a goal with tasks and edges. Verify `get_full_graph` returns a `WorkGraph` containing all nodes and edges.

Use `Database::open_in_memory()`. Create a helper to build test nodes with sensible defaults.

**Verification:**
Run: `cargo test graph_store_test`
Expected: All tests pass

**Commit:** `feat(graph): SqliteGraphStore with node/edge CRUD, subtree queries`

<!-- END_TASK_4 -->
<!-- END_SUBCOMPONENT_B -->

<!-- START_SUBCOMPONENT_C (tasks 5-6) -->

<!-- START_TASK_5 -->
### Task 5: Dependency resolution and task surfacing

**Verifies:** P1b.AC4.1, P1b.AC4.2, P1b.AC4.3

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/dependency.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/store.rs` — implement `get_ready_tasks` and `get_next_task`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_dependency_test.rs`

**Implementation:**

`src/graph/dependency.rs`:
- `pub fn check_dependencies_met(conn: &rusqlite::Connection, node_id: &str) -> rusqlite::Result<bool>` — query all DependsOn edges from this node, check if all target nodes have status Completed. Returns true if all deps are met (or no deps exist).

In `SqliteGraphStore`:
- **get_ready_tasks**: Query task nodes under goal where status = 'ready'. A task is Ready when it was moved there by the status update logic (see below).
- **get_next_task**: From ready tasks, sort by: (1) priority (Critical > High > Medium > Low), (2) downstream count (COUNT of nodes that transitively DependsOn this task), (3) break ties by created_at. Return the first.

Status transition hook: When `update_node` completes a task (status -> Completed), scan all nodes that DependsOn it. For each, if all DependsOn targets are now Completed and current status is Pending, update to Ready. This runs within the same `BEGIN IMMEDIATE` transaction as the status update.

**Testing:**

Tests must verify:
- P1b.AC4.1: Create task A (Pending) and task B (Pending, DependsOn A). Complete A. Verify B is now Ready.
- P1b.AC4.2: Create 3 tasks under a goal — one Ready, one Pending (dep not met), one Completed. `get_ready_tasks` returns only the Ready one.
- P1b.AC4.3: Create 2 Ready tasks — one High priority blocking 3 downstream tasks, one Critical priority blocking 0. `get_next_task` returns the Critical one (priority wins over downstream count).

**Verification:**
Run: `cargo test graph_dependency_test`
Expected: All tests pass

**Commit:** `feat(graph): dependency resolution, ready surfacing, next task recommendation`

<!-- END_TASK_5 -->

<!-- START_TASK_6 -->
### Task 6: Atomic task claiming and FTS5 search

**Verifies:** P1b.AC5.1, P1b.AC5.2, P1b.AC6.1, P1b.AC6.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/graph/store.rs` — implement `claim_task` and `search_nodes`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_claim_search_test.rs`

**Implementation:**

**claim_task**: Single conditional UPDATE within `BEGIN IMMEDIATE`:
```sql
UPDATE nodes SET status = 'claimed', assigned_to = ?1, started_at = ?2
WHERE id = ?3 AND status = 'ready';
```
Check `conn.changes() == 1`. If so, return `Ok(true)`. If 0, return `Ok(false)`.

**search_nodes**: FTS5 MATCH query:
```sql
SELECT n.* FROM nodes n
JOIN nodes_fts fts ON n.rowid = fts.rowid
WHERE nodes_fts MATCH ?1
```
Add optional WHERE clauses for `project_id` and `node_type` filters. Add `LIMIT` clause.

Note: The FTS5 sync triggers (created in Phase 1a schema) keep `nodes_fts` in sync automatically. No application code needed for sync.

**Testing:**

Tests must verify:
- P1b.AC5.1: Create a Ready task, `claim_task(id, "agent-1")` returns true. Node now has status Claimed and assigned_to = "agent-1".
- P1b.AC5.2: Create a Ready task, claim it once (true), claim it again (false — already claimed).
- P1b.AC6.1: Create nodes with various titles/descriptions. `search_nodes("authentication")` returns nodes containing that term.
- P1b.AC6.2: Create nodes in different projects. Search with `project_id` filter returns only nodes from that project. Create nodes of different types (e.g., Task and Observation). Search with `node_type` filter returns only nodes of that type.

**Verification:**
Run: `cargo test graph_claim_search_test`
Expected: All tests pass

**Commit:** `feat(graph): atomic task claiming and FTS5 full-text search`

<!-- END_TASK_6 -->
<!-- END_SUBCOMPONENT_C -->

<!-- START_SUBCOMPONENT_D (tasks 7-8) -->

<!-- START_TASK_7 -->
### Task 7: Graph tools for agents — low-level and high-level

**Verifies:** P1b.AC7.1, P1b.AC7.2

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/graph_tools.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/mod.rs` — add `pub mod graph_tools;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_tools_test.rs`

**Implementation:**

Each tool implements the existing `Tool` trait (async_trait, name/description/parameters/execute). Tools receive a `SqliteGraphStore` (via `Arc`) at construction time.

**Low-level tools** (thin wrappers around GraphStore):
- `CreateNodeTool` — params: `{ node_type, title, description, parent_id?, priority?, metadata? }`. If `parent_id` is given, generates child ID using `next_child_seq`. If not, generates goal ID. Creates the Contains edge if parent exists.
- `UpdateNodeTool` — params: `{ node_id, status?, title?, description?, metadata? }`. Validates status transitions.
- `AddEdgeTool` — params: `{ edge_type, from_node, to_node, label? }`. Generates edge ID.
- `QueryNodesTool` — params: `{ node_type?, status?, project_id?, parent_id?, query? }`. Returns JSON array of matching nodes.
- `SearchNodesTool` — params: `{ query, project_id?, node_type?, limit? }`. FTS5 search, returns JSON results.

**High-level tools** (workflow shortcuts composing multiple store operations):
- `ClaimTaskTool` — params: `{ node_id }`. Calls `claim_task`.
- `LogDecisionTool` — params: `{ title, description, options: [{ title, description, pros?, cons? }], parent_id? }`. Creates Decision node + Option nodes + LeadsTo edges in one call.
- `ChooseOptionTool` — params: `{ decision_id, option_id, rationale }`. Adds Chosen edge, Rejected edges to other options, updates Decision status to Decided.
- `RecordOutcomeTool` — params: `{ parent_id, title, description, success }`. Creates Outcome node + LeadsTo edge.
- `RecordObservationTool` — params: `{ title, description, related_node_id? }`. Creates Observation node + Informs edge if related_node_id given.
- `RevisitTool` — params: `{ outcome_id, reason, new_decision_title? }`. Creates Revisit node + LeadsTo edge. If new_decision_title given, creates new Decision node + LeadsTo edge from Revisit.

Each tool's `parameters()` method returns a JSON schema describing its params.
Each tool's `execute()` method parses params from `serde_json::Value`, calls the store, and returns a JSON string result.

**Testing:**

Tests must verify:
- P1b.AC7.1: Create a node via `CreateNodeTool::execute()`, verify it exists in the store. Same for update, add_edge, query, search.
- P1b.AC7.2: Use `LogDecisionTool` to create a decision with 2 options, verify Decision + 2 Option nodes + 2 LeadsTo edges created. Use `ChooseOptionTool` to pick one, verify Chosen/Rejected edges and status updates. Use `RecordOutcomeTool`, verify Outcome + LeadsTo edge. Use `RecordObservationTool`, verify Observation + Informs edge.

Use `Database::open_in_memory()` and construct tools with `Arc<SqliteGraphStore>`.

**Verification:**
Run: `cargo test graph_tools_test`
Expected: All tests pass

**Commit:** `feat(tools): graph tools for agents — create, update, query, search, claim, decision workflow`

<!-- END_TASK_7 -->

<!-- START_TASK_8 -->
### Task 8: Wire up graph CLI commands

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add `Tasks`, `Decisions`, `Status`, `Search` subcommands

**Implementation:**

Add new `Commands` variants:

```rust
/// View and manage tasks
Tasks {
    #[command(subcommand)]
    action: Option<TaskAction>,
},
/// View and manage decisions
Decisions {
    #[command(subcommand)]
    action: Option<DecisionAction>,
},
/// Show project status
Status,
/// Search graph nodes
Search {
    /// Search query
    query: String,
},
```

```rust
#[derive(Subcommand)]
enum TaskAction {
    /// List all tasks (filterable)
    List {
        #[arg(long)] status: Option<String>,
        #[arg(long)] priority: Option<String>,
    },
    /// Show ready tasks
    Ready,
    /// Recommend next task
    Next,
    /// Show task tree
    Tree,
}

#[derive(Subcommand)]
enum DecisionAction {
    /// List active decisions
    List,
    /// Current truth — active decisions only
    Now,
    /// Full evolution including abandoned paths
    History,
    /// Show decision details
    Show { id: String },
}
```

Each command:
1. Resolves project via `--project` flag or cwd
2. Opens database, creates `SqliteGraphStore`
3. Calls the appropriate store method
4. Formats and prints results

Keep formatting simple — structured text output. Fancy formatting is not a priority.

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- tasks --help`
Expected: Shows task subcommands (list, ready, next, tree)

Run: `cargo run -- decisions --help`
Expected: Shows decision subcommands (list, now, history, show)

**Commit:** `feat(cli): tasks, decisions, status, and search commands`

<!-- END_TASK_8 -->
<!-- END_SUBCOMPONENT_D -->

<!-- START_TASK_9 -->
### Task 9: Concurrency test — multiple claim_task attempts

**Verifies:** P1b.AC5.1, P1b.AC5.2 (concurrency aspect)

**Files:**
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/graph_concurrency_test.rs`

**Implementation:**

Write a concurrency test that spawns multiple tokio tasks all trying to claim the same Ready task simultaneously. Verify that exactly one succeeds and the rest get false.

```rust
#[tokio::test]
async fn test_concurrent_task_claiming() {
    // Setup: create a goal with one Ready task
    // Spawn 10 tokio tasks all calling claim_task for the same node
    // Collect results
    // Assert exactly 1 true, 9 false
}
```

This tests the atomicity guarantee of the conditional UPDATE under concurrent access through the single tokio-rusqlite connection.

**Testing:**

- Exactly one of N concurrent claim attempts succeeds
- The task ends up with status Claimed and a single assigned_to

**Verification:**
Run: `cargo test graph_concurrency_test`
Expected: All tests pass

**Commit:** `test(graph): concurrent task claiming atomicity`

<!-- END_TASK_9 -->
