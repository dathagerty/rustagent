# Rustagent v2: Autonomous Multi-Agent Coding System

## Vision
Clean-slate redesign of rustagent as an autonomous coding agent with multi-agent orchestration, task tracking, decision graphs, AGENTS.md support, and configurable autonomy.

Inspired by: **Deciduous** (decision graphs), **Chainlink** (session-based task tracking with handoff notes), **Beads** (hash-based hierarchical IDs, dependency-aware task graphs, `ready` surfacing).

### Relationship to V1

V2 is a **clean break**, not a migration. The v1 JSON spec format, planning agent, Ralph loop, and ratatui TUI are all superseded. There is no import tooling or backward compatibility — v1 was only used by the author. Selected modules are cherry-picked into v2 (LLM clients, security, tools) as listed in the "Files to Cherry-Pick" section. The TUI is replaced entirely by the web UI.

## Key Design Decisions

- **Tokio channels over actor frameworks** - Agent count is small (<10), broadcast + per-agent mpsc
- **SQLite for all persistence** - Single binary, WAL mode for concurrent reads
- **FTS5 for full-text search** - SQLite built-in, no external dependencies; covers graph node search without embedding infrastructure
- **Agent profiles in TOML config** - Customizable without recompiling; builtin defaults as fallbacks
- **Hybrid messaging** - Orchestrator controls lifecycle + agents can message peers directly
- **Unified work graph** - Tasks, decisions, outcomes, and observations are all nodes in a single DAG. 7 node types, 7 edge types. Eliminates duplication between separate task and decision systems (inspired by Deciduous, Chainlink, Beads)
- **Session model from Chainlink** - Handoff notes preserve context across sessions (temporal, separate from graph)
- **Hash-based IDs from Beads** - Merge-safe, hierarchical (goal.task.subtask)

---

## Daemon + Web UI Architecture

### Dual-Mode Operation

Rustagent supports two modes:

1. **Standalone CLI**: Direct execution for one-off commands, scripting, CI. Works without a running daemon.
2. **Daemon mode**: Long-running background process with HTTP API + WebSocket. Required for the web UI and for long-running orchestration.

The CLI auto-detects whether a daemon is running (via PID file / health check) and routes commands accordingly:
- Daemon running → CLI becomes thin client, sends API requests
- No daemon → CLI executes directly (standalone mode)

### Daemon

```
rustagent daemon start           # Start daemon in background
rustagent daemon stop            # Stop daemon
rustagent daemon status          # Check if daemon is running
rustagent daemon logs            # Tail daemon logs
```

The daemon is the same Rust binary with a `daemon` subcommand. It:
- Starts an HTTP server (axum) on a configurable port (default: `127.0.0.1:7400`)
- Runs the orchestrator for active goals
- Exposes REST API for CRUD operations
- Exposes WebSocket endpoint for real-time updates
- Writes PID file to `~/.local/share/rustagent/rustagent.pid`
- Logs to `~/.local/state/rustagent/logs/` (same as current)

```rust
// src/daemon/mod.rs

pub struct Daemon {
    config: Config,
    db: Arc<Database>,
    orchestrators: HashMap<String, Orchestrator>,  // One per active goal
    ws_broadcaster: broadcast::Sender<WsEvent>,
}
```

### HTTP API

```
# Projects
GET    /api/projects                  # List all projects
POST   /api/projects                  # Register a project
GET    /api/projects/:id              # Get project details
DELETE /api/projects/:id              # Remove project

# Work Graph (unified nodes + edges)
GET    /api/projects/:id/goals        # List goal nodes for project
POST   /api/projects/:id/goals        # Create a goal (starts orchestration)
GET    /api/nodes/:id                 # Get any node with its edges
PATCH  /api/nodes/:id                 # Update node (status, metadata)
POST   /api/nodes/:id/children        # Create child node
GET    /api/goals/:id/tree            # Get full node tree under goal

# Task Views (projections of the work graph)
GET    /api/goals/:id/tasks           # List task nodes for goal
GET    /api/goals/:id/tasks/ready     # Get ready task nodes
GET    /api/goals/:id/tasks/next      # Get recommended next task

# Decision Views (projections of the work graph)
GET    /api/projects/:id/decisions         # Active decisions (now mode)
GET    /api/projects/:id/decisions/history  # Full decision graph (history mode)
POST   /api/projects/:id/decisions/export   # Export ADRs to project dir

# Graph Import/Export
GET    /api/projects/:id/graph/export       # Export all goals as TOML
GET    /api/goals/:id/export                # Export single goal as TOML
POST   /api/projects/:id/graph/import       # Import TOML (body: file content)
POST   /api/projects/:id/graph/diff         # Diff TOML against DB state

# Sessions
GET    /api/goals/:id/sessions        # List sessions
GET    /api/sessions/:id              # Get session with handoff notes

# Search (full-text search over graph nodes)
POST   /api/projects/:id/search        # FTS5 search over node titles/descriptions

# Agents (real-time)
GET    /api/goals/:id/agents          # List active agents for goal

# WebSocket
WS     /ws                            # Real-time event stream
```

### WebSocket Events

```typescript
type WsEvent =
  | { type: "agent_spawned"; agentId: string; profile: string; goalId: string }
  | { type: "agent_progress"; agentId: string; turn: number; summary: string }
  | { type: "agent_completed"; agentId: string; outcome: AgentOutcome }
  | { type: "node_created"; node: GraphNode }
  | { type: "node_status_changed"; nodeId: string; nodeType: string; oldStatus: string; newStatus: string }
  | { type: "edge_created"; edge: GraphEdge }
  | { type: "session_ended"; sessionId: string; handoffNotes: string }
  | { type: "tool_execution"; agentId: string; tool: string; args: object; result: string }
  | { type: "orchestrator_state_changed"; goalId: string; state: string }
```

### Web UI

**Stack**: TypeScript + Bun + Vite + Svelte 5

**Rationale**: Svelte for minimal boilerplate, built-in reactivity (runes — no separate state management library needed), and smallest runtime footprint. WebSocket-driven updates integrate naturally with Svelte's reactive stores. Graph visualization uses Cytoscape.js (framework-agnostic, handles pan/zoom/drag/expand-collapse for interactive decision and goal graph views). Bun is build toolchain only (via Vite); runtime is the browser. No SSR — this is a locally-served SPA.

**Location**: `web/` directory in the repo (separate from `src/`)

```
web/
├── package.json
├── svelte.config.js
├── tsconfig.json
├── vite.config.ts
├── bun.lock
├── index.html
├── src/
│   ├── main.ts                    # Entry point
│   ├── App.svelte                 # Root component + routing
│   ├── api/
│   │   ├── client.ts              # HTTP API client
│   │   └── websocket.ts           # WebSocket connection + event handling
│   ├── stores/                    # Svelte runes-based reactive stores
│   │   ├── projects.svelte.ts
│   │   ├── graph.svelte.ts       # Unified: nodes, edges, goals, tasks, decisions
│   │   ├── agents.svelte.ts
│   │   └── search.svelte.ts      # Full-text search over graph nodes
│   ├── views/
│   │   ├── Dashboard.svelte       # Overview: active goals, agent status, recent activity
│   │   ├── ProjectList.svelte     # All projects
│   │   ├── ProjectDetail.svelte   # Goals, tasks, decisions for a project
│   │   ├── TaskTree.svelte        # Projection: task nodes with hierarchy and status
│   │   ├── DecisionGraph.svelte   # Projection: decision/option/outcome nodes (now + history modes)
│   │   ├── GraphSearch.svelte     # Full-text search across graph nodes
│   │   ├── AgentMonitor.svelte    # Real-time agent activity (tool calls, progress)
│   │   └── SessionHistory.svelte  # Past sessions with handoff notes
│   ├── components/
│   │   ├── GraphNodeCard.svelte
│   │   ├── CytoscapeGraph.svelte  # Wrapper for Cytoscape.js (pan/zoom/drag graph views)
│   │   ├── AgentStatusBadge.svelte
│   │   ├── SearchResult.svelte
│   │   └── ...
│   └── styles/
└── public/
```

**Key views:**

- **Dashboard**: At-a-glance view of all active goals across projects, running agents, task completion %, recent decisions
- **Task Tree**: Projection of the work graph showing goal → tasks → subtasks with dependency edges, status colors, agent assignments
- **Decision Graph**: Projection of the work graph filtering to decision/option/outcome/revisit nodes. Toggle between Now mode (active decisions only) and History mode (full evolution). Click nodes to see details.
- **Agent Monitor**: Real-time feed of agent activity - tool calls, file changes, progress reports. Like watching multiple terminal sessions.
- **Graph Search**: Full-text search across all graph nodes (observations, outcomes, decisions). Filter by node type, project, status.

### Serving the Web UI

Two modes, controlled by a Cargo feature flag:

1. **Development** (default): No frontend build during `cargo build`. Developers run `bun run dev` in `web/` for Vite's dev server with HMR, proxying API calls to the daemon. The daemon does not serve the UI — if no embedded assets exist and no `web/dist/` is found, `/*` returns a message directing the user to start the Vite dev server or build with `--features bundle-ui`.

2. **Release / single-binary** (`--features bundle-ui`): `build.rs` runs `bun install && bun run build` in `web/`, then `rust-embed` compiles `web/dist/` into the binary. The daemon serves embedded assets directly — no external files needed. One binary, fully self-contained.

**`build.rs`** (frontend build, only with `bundle-ui` feature):

```rust
fn main() {
    #[cfg(feature = "bundle-ui")]
    {
        println!("cargo:rerun-if-changed=web/src");
        println!("cargo:rerun-if-changed=web/package.json");

        let web_dir = "web";

        let status = std::process::Command::new("bun")
            .args(["install"])
            .current_dir(web_dir)
            .status()
            .expect("bun must be installed to build with bundle-ui");
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

**Embedded assets** (behind `bundle-ui` feature):

```rust
#[cfg(feature = "bundle-ui")]
#[derive(rust_embed::Embed)]
#[folder = "web/dist/"]
struct UiAssets;
```

The daemon's axum fallback handler tries `UiAssets::get(path)`, falling back to `UiAssets::get("index.html")` for SPA routing. Content types are inferred by `rust-embed`.

**The daemon's axum server serves:**
- `/api/*` → REST API
- `/ws` → WebSocket
- `/*` → Embedded UI assets (with `bundle-ui`) or "UI not bundled" message (without)

### New Rust Dependencies for Daemon

```toml
axum = { version = "0.8", features = ["ws"] }         # HTTP server + WebSocket
tower = "0.5"                                           # Middleware
tower-http = { version = "0.6", features = ["cors", "fs"] }  # CORS + static files
rust-embed = { version = "8", features = ["axum"], optional = true }  # Static asset embedding
```

---

## Multi-Project Architecture

### Central Database

All data lives in a single SQLite database at `~/.local/share/rustagent/rustagent.db` (XDG data dir). This enables:
- Cross-project search (observations from project A are findable when working on project B)
- Unified task/decision views across all projects
- Single source of truth

### Project Registration

Projects are explicitly registered:

```
rustagent project add my-api /Users/david/code/my-api
rustagent project add frontend /Users/david/code/frontend
rustagent project list
rustagent project remove my-api
rustagent project show my-api
```

```rust
pub struct Project {
    pub id: String,           // Auto-generated hash (ra-xxxx)
    pub name: String,         // Friendly name (e.g., "my-api")
    pub path: PathBuf,        // Absolute path to project root
    pub registered_at: DateTime<Utc>,
    pub config_overrides: Option<ProjectConfig>,  // Per-project config
    pub metadata: HashMap<String, String>,
}
```

**Project resolution**: When running commands, specify project by name:
```
rustagent run --project my-api "Add user authentication"
rustagent tasks --project my-api
rustagent status --project my-api
```

If no `--project` flag, rustagent looks for a registered project matching the current working directory.

### Data Scoping

All entities are scoped to a project via `nodes.project_id`:
- Goals, tasks, decisions, options, outcomes — all node types inherit project scope
- Sessions also carry `project_id`
- Queries default to the current project
- Full-text search can optionally span all projects (for cross-project learnings)

### ADR Export

Despite the central database, ADR export writes to the project's directory:
```
rustagent decisions export --project my-api
# → /Users/david/code/my-api/decisions/001-auth-approach.md
```

### Database Schema Addition

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    registered_at TEXT NOT NULL,
    config_overrides TEXT,  -- JSON
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Other tables with project_id:
-- nodes.project_id REFERENCES projects(id)       (all node types: goals, tasks, decisions, etc.)
-- sessions.project_id REFERENCES projects(id)
```

---

## Database Concurrency Strategy

### Single Connection, WAL Mode

All database access goes through a single `tokio_rusqlite::Connection` shared via `Arc`. This is the entire write serialization strategy — `tokio-rusqlite` runs one SQLite connection on a dedicated background thread and processes `.call()` closures sequentially through an internal channel. No additional write queue or mutex is needed.

**Configuration at connection open:**
```sql
PRAGMA journal_mode = WAL;          -- Concurrent reads, serialized writes
PRAGMA busy_timeout = 5000;         -- 5s safety net (shouldn't trigger with single connection)
PRAGMA foreign_keys = ON;
PRAGMA wal_autocheckpoint = 1000;   -- Default, tune only if profiling shows need
```

### Transaction Discipline

All write transactions use `BEGIN IMMEDIATE` to prevent the reader-to-writer upgrade deadlock (where a `DEFERRED` transaction that starts reading, then tries to write, gets an immediate `SQLITE_BUSY` that `busy_timeout` cannot help with).

```rust
conn.call(|conn| {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // ... writes ...
    tx.commit()?;
    Ok(())
}).await?;
```

### Atomic Task Claiming

`claim_task` is implemented as a single conditional UPDATE — no read-then-write race:

```sql
UPDATE nodes SET status = 'claimed', assigned_to = ?1, started_at = ?2
WHERE id = ?3 AND status = 'ready';
```

If `changes() == 1`, the claim succeeded. If `changes() == 0`, another worker got there first. The caller retries with a different ready task.

### Future Scaling

At <10 agents, a single connection is sufficient. If read latency ever becomes an issue, the first optimization would be a second read-only connection (WAL allows concurrent readers alongside one writer). This is an optimization to add if profiling shows need, not an upfront design requirement.

---

## Unified Work Graph

### Core Model

The work graph is a single directed acyclic graph (DAG) that unifies task tracking and decision recording. Every entity — goals, tasks, decisions, options, outcomes, observations, and pivots — is a node in the same graph. Relationships between them are edges.

This eliminates the duplication between separate task and decision systems: dependencies, blocking, hierarchy, and cross-references are all expressed once through edges. Task trees and decision histories are different views (projections) of the same underlying graph.

### IDs

Hierarchical IDs with sibling-unique segments (inspired by Beads):
- `ra-a3f8` (Goal)
- `ra-a3f8.1` (Task or Decision under that goal)
- `ra-a3f8.1.3` (Subtask or Option)

The **full dotted path is the primary key** (e.g., `ra-a3f8.1.3`). Each segment only needs to be unique among its siblings under one parent, not globally unique. This means:

- **Goal IDs**: `ra-` prefix + 4 hex chars from UUID v4. With 65,536 possibilities, collision at 50% requires ~301 *goals* — more than enough headroom.
- **Child IDs**: Sequential integer counter per parent (`.1`, `.2`, `.3`). The parent node serializes child creation, so no concurrency concern. Counter is stored on the parent node in metadata (`next_child_seq`).
- **Primary key**: The full path string (`ra-a3f8.1.3`). Globally unique by construction since each segment is sibling-unique and the path encodes the full hierarchy.

This keeps IDs short, human-friendly, and collision-safe at any scale. The hierarchical structure is encoded directly in the ID rather than being a display convenience — `Contains` edges still exist for queryability, but the ID itself is authoritative for parentage.

**Edge IDs**: `e-` prefix + 8 hex chars from UUID v4 (e.g., `e-a3f8b2c1`). Edges are global, not hierarchical, so they use a flat namespace with enough entropy to avoid collisions.

### Node Types

7 node types cover the full workflow:

```rust
// src/graph/mod.rs

pub enum NodeType {
    Goal,         // High-level objective driving all work
    Task,         // Work item to be executed by an agent
    Decision,     // Choice point where alternatives are evaluated
    Option,       // Specific approach considered for a decision
    Outcome,      // Result of completed work or action
    Observation,  // Discovery or insight during development
    Revisit,      // Pivot point — abandoned approach leads to new decision
}
```

**Goal**: Top-level objective. Contains Tasks and Decisions via `Contains` edges. Has a priority.

**Task**: Concrete work item. Can contain subtasks (also Task nodes). The workhorse — agents claim and execute these. Carries acceptance criteria and assignment in metadata.

**Decision**: A choice point. Connected to Option nodes via `LeadsTo` edges. Resolved when an option is chosen.

**Option**: An alternative considered for a Decision. Carries pros/cons in metadata. Status becomes `Chosen` or `Rejected`.

**Outcome**: The result of completing a Task or choosing an Option. Records success/failure and what happened.

**Observation**: An insight or discovery. Connected to any node via `Informs` edges. Replaces the old `learnings` field — observations are first-class graph citizens.

**Revisit**: A pivot point created when an Outcome is bad. Connects the failed path to a new Decision, preserving the full reasoning chain.

### Edge Types

7 edge types express all relationships:

```rust
pub enum EdgeType {
    Contains,     // Hierarchical: Goal → Task, Task → Subtask, Goal → Decision
    DependsOn,    // Sequencing: B depends on A (A must complete first)
    LeadsTo,      // Narrative: Decision → Options, Task → Outcome, Outcome → Revisit
    Chosen,       // Selection: Decision → the selected Option
    Rejected,     // Selection: Decision → a rejected Option (label has reason)
    Supersedes,   // Evolution: new node replaces old
    Informs,      // Context: Observation/Outcome provides context to another node
}

pub struct GraphEdge {
    pub id: String,
    pub edge_type: EdgeType,
    pub from_node: String,
    pub to_node: String,
    pub label: Option<String>,  // e.g., rejection reason, dependency description
    pub created_at: DateTime<Utc>,
}
```

### Node Status

A single status enum with type-appropriate semantics:

```rust
pub enum NodeStatus {
    // Lifecycle (all node types)
    Pending,       // Created, not yet actionable
    Active,        // Currently relevant
    Completed,     // Done successfully
    Cancelled,     // No longer needed

    // Task workflow
    Ready,         // All dependencies met, available for claiming
    Claimed,       // Agent has atomically claimed this
    InProgress,    // Active work underway
    Review,        // Work done, awaiting review
    Blocked,       // Cannot proceed (blocked_reason set)
    Failed,        // Attempted and failed

    // Decision workflow
    Decided,       // Decision resolved (a chosen option exists)
    Superseded,    // Replaced by newer approach
    Abandoned,     // Tried and rejected

    // Option workflow
    Chosen,        // This option was selected
    Rejected,      // This option was not selected
}

pub enum Priority {
    Critical,   // Must do immediately
    High,       // Should do soon
    Medium,     // Normal priority
    Low,        // Nice to have
}
```

Not all statuses apply to all node types. Validation rules:
- **Goal**: Pending, Active, Completed, Cancelled
- **Task**: Pending, Ready, Claimed, InProgress, Review, Completed, Blocked, Failed, Cancelled
- **Decision**: Pending, Active, Decided, Superseded
- **Option**: Pending, Active, Chosen, Rejected, Abandoned
- **Outcome**: Active, Completed
- **Observation**: Active
- **Revisit**: Active, Completed

### The Unified Node

```rust
pub struct GraphNode {
    pub id: String,                     // Hash-based: ra-xxxx, ra-xxxx.1, etc.
    pub project_id: String,
    pub node_type: NodeType,
    pub title: String,
    pub description: String,
    pub status: NodeStatus,
    pub priority: Option<Priority>,     // Goals and Tasks
    pub assigned_to: Option<AgentId>,   // Tasks
    pub created_by: Option<AgentId>,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub blocked_reason: Option<String>,
    pub metadata: HashMap<String, String>,  // Type-specific data
}
```

**Metadata by node type:**
- **Task**: `acceptance_criteria` (JSON array), `estimated_complexity`
- **Option**: `pros` (JSON array), `cons` (JSON array)
- **Outcome**: `success` (bool)
- **Revisit**: `pivot_reason`

### How It All Connects

```
┌──────┐  contains   ┌──────────┐  contains   ┌──────────┐
│ Goal │────────────→│  Task A  │────────────→│Subtask A1│
└──────┘             └──────────┘             └──────────┘
    │                     │
    │ contains       leads_to
    ↓                     ↓
┌──────────┐        ┌──────────┐
│ Decision │        │ Outcome  │──→ (if bad) ──→ Revisit ──→ New Decision
└──────────┘        └──────────┘
    │
    │ leads_to
    ├───────────────────┐
    ↓                   ↓
┌──────────┐      ┌──────────┐
│ Option A │      │ Option B │
│ (chosen) │      │(rejected)│
└──────────┘      └──────────┘

┌──────────┐  depends_on  ┌──────────┐
│  Task B  │─────────────→│  Task A  │
└──────────┘              └──────────┘
```

### Task Lifecycle (State Machine)

Task nodes follow this state machine:

```
                                    ┌──────────┐
                                    │ Cancelled│
                                    └──────────┘
                                         ↑
┌─────────┐    deps met    ┌───────┐  agent   ┌─────────┐
│ Pending  │──────────────→│ Ready │─claims──→│ Claimed │
└─────────┘                └───────┘          └─────────┘
                              ↑                    │
                              │               work starts
                              │                    ↓
                         unblocked          ┌────────────┐
                              │             │ InProgress  │
                              │             └────────────┘
                         ┌─────────┐            │      │
                         │ Blocked │←───────────┘      │
                         └─────────┘         work done │
                                                       ↓
                                              ┌────────┐
                         ┌─────────┐          │ Review │
                         │ Failed  │←─────────┴────────┘
                         └─────────┘               │
                                              approved
                                                   ↓
                                            ┌──────────┐
                                            │ Complete  │
                                            └──────────┘
```

Key transitions:
- **Pending → Ready**: Automatic when all `DependsOn` edges point to Completed nodes
- **Ready → Claimed**: Atomic (sets assigned_to + status) — prevents two agents claiming same task
- **Claimed → InProgress**: Agent begins work
- **InProgress → Blocked**: Agent encounters blocker, creates Observation node explaining why
- **Blocked → Ready**: Blocker resolved, re-enters the queue
- **InProgress → Review**: Work complete, optional review gate
- **Review → Complete**: Reviewer approves (or auto-complete if no review gate)
- **Review → Failed**: Reviewer rejects, needs rework
- **Any → Cancelled**: Goal changed, task no longer needed

### Decision Workflow (Forward Logging)

Agents follow a forward-logging pattern (from Deciduous):

1. **Log intention**: Create Decision node under current Goal (via `Contains` edge), add Option nodes via `LeadsTo` edges
2. **Choose**: Add `Chosen` edge from Decision to selected Option, `Rejected` edges (with reason labels) to others. Decision status → `Decided`.
3. **Execute**: Chosen Option may generate Task nodes (via `Contains` edges from the Goal)
4. **Record outcome**: Task completion creates Outcome node (via `LeadsTo` edge from Task)
5. **Pivot if needed**: Bad Outcome → create Revisit node (via `LeadsTo`) → new Decision node (via `LeadsTo`)

This creates an unbroken reasoning chain explaining why the codebase looks the way it does.

### When Decisions Are Created

Agents create decision records when:
- **Architectural choices**: Picking patterns, libraries, data structures
- **Trade-off moments**: Performance vs. readability, simplicity vs. flexibility
- **Multiple valid approaches**: Agent considers 2+ options before choosing
- **Blockers encountered**: Why something was blocked and what was tried
- **Pivots**: Abandoning one approach for another (Revisit node)

The orchestrator can also require decisions at approval gates (configurable).

### ADR Export

Despite the unified graph, ADR export works by filtering to Decision nodes and gathering their connected Options (Chosen/Rejected), Outcomes, and related Tasks:

```
decisions/
├── 001-authentication-approach.md
├── 002-database-choice.md
└── 003-api-design.md
```

Each ADR follows the format:
```markdown
# ADR-001: Authentication Approach

## Status: Active
## Context: [from Decision node description]
## Options Considered:
- **Option A**: [description] - CHOSEN
  - Pros: ...
  - Cons: ...
- **Option B**: [description] - REJECTED
  - Reason: ...
## Outcome: [from Outcome node]
## Related Tasks: ra-a3f8.2, ra-a3f8.3
```

### Two Decision Visualization Modes (from Deciduous)

**Now Mode** (`rustagent decisions now`): Filter the graph to Active/Decided Decision and Chosen Option nodes. Shows how the system works today.

**History Mode** (`rustagent decisions history`): Full graph including Abandoned, Superseded, and Rejected nodes. Explains why things changed.

### Session Model (from Chainlink)

Sessions are separate from the graph — they're temporal, not structural:

```rust
pub struct Session {
    pub id: String,
    pub goal_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub handoff_notes: Option<String>,  // Generated at session end
    pub agent_ids: Vec<AgentId>,        // Agents that participated
    pub summary: Option<String>,        // Auto-generated session summary
}
```

Each `rustagent run` creates a session. When a session ends (goal complete, user interrupt, error):
1. Orchestrator generates handoff notes **deterministically from graph state** (no LLM call). The template queries nodes by status and formats them:

```
## Done
- ra-a3f8.1: Design auth schema (completed)
- ra-a3f8.2: JWT vs sessions → JWT chosen

## Remaining
- ra-a3f8.3: Implement JWT middleware (ready)
- ra-a3f8.4: Add refresh token rotation (pending, blocked by ra-a3f8.3)

## Blocked
- None

## Decisions Made
- ra-a3f8.2: JWT tokens chosen over server-side sessions (stateless, scalable)
```

2. Handoff notes are stored in the session record
3. Next `rustagent run` on the same goal loads previous handoff notes into worker context

### Smart Task Surfacing

**`ready` command** (from Beads): Query Task nodes where all `DependsOn` targets are Completed and status is Ready.

**`next` command** (from Chainlink): Recommend the highest-priority Ready task, considering:
1. Priority level (Critical > High > Medium > Low)
2. Number of downstream nodes it unblocks (prefer tasks that unblock the most)
3. Estimated complexity (simpler tasks first for momentum)

### Node Decay for Context (from Beads)

Old completed nodes are compacted when injected into agent context to save tokens:
- Recent nodes (< 7 days): full detail (description, criteria, outcomes)
- Older nodes (7-30 days): summary only (title, status, key outcomes)
- Ancient nodes (> 30 days): just title and status
- Thresholds configurable in `.rustagent/config.toml`
- Full detail is always available via `query_nodes` regardless of age

### User Visibility

```
rustagent status              # Goal tree + agent states + session info
rustagent tasks               # List task nodes (filterable by status, label, priority)
rustagent tasks ready         # Show ready-to-claim tasks
rustagent tasks next          # Recommend next task
rustagent tasks tree          # Tree view: goal → tasks → subtasks
rustagent decisions           # List active decisions
rustagent decisions now       # Current truth — active decisions only
rustagent decisions history   # Full evolution including abandoned paths
rustagent decisions show <id> # Show decision with options and outcome
rustagent decisions export    # Export as markdown ADR files
rustagent sessions            # List sessions with handoff notes
rustagent sessions latest     # Show most recent session's handoff notes
```

---

## Work Graph Import/Export

### Motivation

The canonical work graph lives in SQLite, but collaborators need to:
- Share graph state through git (review task breakdowns and decisions in PRs)
- Hand off work between team members (beyond just handoff notes)
- Back up and restore graph state
- Resolve divergent graph states after independent work

### Format: TOML, One File Per Goal

**Why TOML**: Excellent Rust support (`toml` crate + serde), human-readable, unambiguous spec, typed values. Table-per-entity maps naturally to git-friendly diffs where each node/edge is an independent hunk.

**Why one file per goal**: Goals are the natural collaboration boundary. Independent goals produce independent files with zero merge conflicts across goals. File-level git operations (blame, log) work well at this granularity.

### File Structure

```
.rustagent/
├── config.toml                   # Project config (autonomy level, default profile, etc.)
├── profiles/                     # Custom agent profiles (version-controlled)
│   └── rust-coder.toml
├── graph/
│   ├── ra-a3f8.toml              # Goal: "Add user authentication"
│   └── ra-b2c1.toml              # Goal: "Optimize database queries"
└── sessions/
    ├── ra-a3f8-2025-01-15.toml
    └── ra-a3f8-2025-01-16.toml
```

The `.rustagent/` directory lives at the project root and is intended to be committed to git (like `.github/`).

### Goal File Format

```toml
[meta]
version = 1
goal_id = "ra-a3f8"
project = "my-api"
exported_at = "2025-01-15T10:30:00Z"
content_hash = "abc123def456"       # Blake3 hash of nodes+edges for quick change detection

# ─── Nodes ──────────────────────────────────────────────
# Each [nodes."<id>"] block is an independent git hunk.
# Sorted by ID for deterministic output.

[nodes."ra-a3f8"]
type = "goal"
title = "Add user authentication"
description = "Implement JWT-based auth with refresh tokens"
status = "active"
priority = "high"
labels = ["security", "mvp"]
created_at = "2025-01-15T10:00:00Z"

[nodes."ra-a3f8.1"]
type = "task"
title = "Design auth schema"
description = "Create database tables for users, tokens, sessions"
status = "completed"
priority = "high"
assigned_to = "coder-1"
created_at = "2025-01-15T10:01:00Z"
started_at = "2025-01-15T10:02:00Z"
completed_at = "2025-01-15T10:30:00Z"

[nodes."ra-a3f8.1".metadata]
acceptance_criteria = [
    "Users table with email + hashed password",
    "Refresh token table with expiry",
]

[nodes."ra-a3f8.2"]
type = "decision"
title = "JWT vs session tokens"
description = "Choose authentication token strategy"
status = "decided"
created_at = "2025-01-15T10:05:00Z"

[nodes."ra-a3f8.2.1"]
type = "option"
title = "JWT tokens"
description = "Stateless JWT with short-lived access + long-lived refresh"
status = "chosen"
created_at = "2025-01-15T10:05:00Z"

[nodes."ra-a3f8.2.1".metadata]
pros = ["Stateless", "Horizontally scalable", "No server-side session store"]
cons = ["Can't revoke individual tokens", "Larger payload than session ID"]

[nodes."ra-a3f8.2.2"]
type = "option"
title = "Server-side sessions"
description = "Traditional session cookie with server-side store"
status = "rejected"
created_at = "2025-01-15T10:05:00Z"

[nodes."ra-a3f8.2.2".metadata]
pros = ["Simple revocation", "Small cookie size"]
cons = ["Requires session store", "Horizontal scaling needs shared store"]

# ─── Edges ──────────────────────────────────────────────
# Each [edges."<id>"] block is an independent git hunk.
# Sorted by ID for deterministic output.

[edges."e-0001"]
type = "contains"
from = "ra-a3f8"
to = "ra-a3f8.1"

[edges."e-0002"]
type = "contains"
from = "ra-a3f8"
to = "ra-a3f8.2"

[edges."e-0003"]
type = "leads_to"
from = "ra-a3f8.2"
to = "ra-a3f8.2.1"

[edges."e-0004"]
type = "leads_to"
from = "ra-a3f8.2"
to = "ra-a3f8.2.2"

[edges."e-0005"]
type = "chosen"
from = "ra-a3f8.2"
to = "ra-a3f8.2.1"

[edges."e-0006"]
type = "rejected"
from = "ra-a3f8.2"
to = "ra-a3f8.2.2"
label = "Requires server-side session store, adds operational complexity"
```

### Session File Format

Sessions export alongside their goal in a separate directory:

```toml
# .rustagent/sessions/ra-a3f8-2025-01-15.toml

[meta]
session_id = "s-1234"
goal_id = "ra-a3f8"
started_at = "2025-01-15T10:00:00Z"
ended_at = "2025-01-15T12:30:00Z"
agents = ["planner-1", "coder-1"]

summary = "Completed auth schema design and JWT decision"

handoff_notes = """
## Done
- Designed auth schema (ra-a3f8.1)
- Decided on JWT tokens (ra-a3f8.2)

## Remaining
- Implement JWT middleware (ra-a3f8.3)
- Add refresh token rotation (ra-a3f8.4)

## Blockers
- None
"""
```

### Git-Friendliness Properties

1. **One table per entity**: Each `[nodes."id"]` and `[edges."id"]` block is an independent git hunk. Adding a node = adding lines at a predictable location (auto-mergeable). Modifying a node = changing lines within one block (conflicts only if the same entity is modified by both sides).

2. **Deterministic ordering**: Nodes and edges sorted lexicographically by ID. Re-exporting unchanged state produces byte-identical output. No spurious diffs.

3. **Hash-based IDs**: Concurrent node creation by different collaborators won't produce ID collisions (UUID v4-based). Two people can independently add tasks to the same goal and merge cleanly.

4. **Content hash**: `content_hash` in `[meta]` enables quick "has anything changed?" checks without diffing the full file. Useful for CI hooks and auto-sync.

5. **Omitted fields**: Null/empty fields are omitted entirely (no `assigned_to = ""` noise). Fields only appear when they carry meaningful data, keeping diffs minimal.

### Conflict Resolution

**Automatic (git handles it):**
- Collaborator A adds `ra-a3f8.3`, Collaborator B adds `ra-a3f8.4` → different hunks, clean merge
- Collaborator A adds edge `e-0007`, Collaborator B adds `e-0008` → different hunks, clean merge
- Collaborator A modifies `ra-a3f8.1`, Collaborator B adds `ra-a3f8.5` → different hunks, clean merge

**Manual (git conflict markers):**
- Both modify the status of `ra-a3f8.1` → standard git conflict on the `status` line
- Both edit the description of the same node → standard conflict, human picks winner

**Import-level conflict resolution:**

When `rustagent graph import` encounters a node that exists in the DB with different content than the file, it applies one of three strategies:

| Mode | New nodes | Changed nodes | Unchanged nodes |
|------|-----------|---------------|-----------------|
| `--merge` (default) | Added | Flagged as conflicts | Skipped |
| `--theirs` | Added | File wins | Skipped |
| `--ours` | Added | DB wins | Skipped |
| `--dry-run` | Shown | Shown | Shown |

Conflict output (in `--merge` mode):
```
$ rustagent graph import .rustagent/graph/ra-a3f8.toml
  Added: ra-a3f8.5 (task: "Add rate limiting")
  Added: e-0009 (depends_on: ra-a3f8.5 → ra-a3f8.3)
  CONFLICT: ra-a3f8.1 status differs (db: in_progress, file: completed)
  Skipped: 4 unchanged nodes, 6 unchanged edges

  1 conflict. Resolve with:
    rustagent graph import --theirs ra-a3f8.toml   # Accept file version
    rustagent graph import --ours ra-a3f8.toml     # Keep DB version
```

### CLI

```
# Export
rustagent graph export                              # Export all goals for current project
rustagent graph export --goal ra-a3f8               # Export specific goal
rustagent graph export --output ./shared/           # Custom output directory
rustagent graph export --sessions                   # Include session files

# Import
rustagent graph import .rustagent/graph/            # Import all goal files in directory
rustagent graph import .rustagent/graph/ra-a3f8.toml  # Import specific file
rustagent graph import --dry-run ra-a3f8.toml       # Preview what would change
rustagent graph import --theirs ra-a3f8.toml        # File wins on conflicts
rustagent graph import --ours ra-a3f8.toml          # DB wins on conflicts

# Diff (compare file state to DB state)
rustagent graph diff .rustagent/graph/ra-a3f8.toml  # Show differences for one goal
rustagent graph diff .rustagent/graph/              # Diff all files against DB
```

### Cross-Goal References

Edges may occasionally cross goal boundaries (e.g., an Observation in Goal A `Informs` a Decision in Goal B). These edges are stored in the file of the `from_node`'s goal.

On import, cross-goal node references are resolved by ID lookup in the DB. If the referenced node doesn't exist, the edge is **skipped with a clear error** — not silently dropped, not deferred:

```
$ rustagent graph import .rustagent/graph/ra-a3f8.toml
  Added: 5 nodes, 8 edges
  SKIPPED: 1 edge (e-0012: informs ra-b2c1.3 — node not found)

  To resolve: import the goal containing ra-b2c1 first, then re-import this file.
```

Re-importing after the referenced goal exists will create the edge (edge creation is idempotent). No deferred state or pending tables — the simplest correct behavior for a rare case.

### Auto-Export Hook

The daemon can optionally auto-export after graph mutations:

```toml
# In rustagent config
[export]
auto_export = true                      # Write .rustagent/graph/ on every graph change
auto_export_sessions = false            # Sessions only on explicit export
auto_export_debounce_ms = 1000          # Batch rapid changes
```

This keeps `.rustagent/graph/` in sync with the DB automatically. Combined with git hooks, teams can enforce that graph state is always committed alongside code changes.

### Implementation Notes

- Uses the `toml` crate with serde `Serialize`/`Deserialize` on graph types
- Custom serializer sorts keys lexicographically within `[nodes.*]` and `[edges.*]` sections
- Export is a pure read: `GraphStore::get_subtree(goal_id)` → TOML serialization → write file
- Import is TOML parse → diff against DB → apply with conflict strategy
- Content hash uses Blake3 over the serialized nodes+edges (excluding `[meta]`)
- Round-trip property: `export → import → export` produces identical files

---

## Agent Orchestration System (Detailed)

Inspired by: **Gastown** (Mayor + ephemeral workers, convoys, recovery-first), **Loom** (state machine coordination, thread persistence), plus first-principles thinking about what LLM-based agents actually need.

### Core Insight: Ephemeral Workers Beat Long-Running Agents

LLM agents degrade with long context windows - they lose focus, hallucinate more, and waste tokens on irrelevant history. The orchestration model embraces this:

- **Workers are ephemeral**: Spawn fresh for each task, execute, report, terminate
- **State lives in SQLite, not in agent memory**: If anything crashes, resume from DB
- **Fresh context = focused work**: Each worker gets only the context it needs

### Architecture: Orchestrator + Worker Pool

```
                        ┌─────────────────────────────┐
                        │       Orchestrator           │
                        │  (persistent, state-machine  │
                        │   based coordination loop)   │
                        └──────────┬──────────────────┘
                                   │
                    ┌──────────────┼──────────────┐
                    │              │              │
              ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼─────┐
              │  Worker A  │ │  Worker B  │ │  Worker C  │
              │ (planner)  │ │  (coder)   │ │  (coder)   │
              │ ephemeral  │ │ ephemeral  │ │ ephemeral  │
              └────────────┘ └────────────┘ └────────────┘
                    │              │              │
                    └──────────────┼──────────────┘
                                   │
                        ┌──────────▼──────────────┐
                        │     SQLite (shared       │
                        │  state: work graph,      │
                        │  sessions)               │
                        └─────────────────────────┘
```

### Orchestrator Design

The orchestrator is NOT an LLM agent. It's a **deterministic state machine** that coordinates work. This is a deliberate choice - the orchestrator doesn't burn tokens on coordination logic; it follows rules.

```rust
// src/agent/orchestrator.rs

pub struct Orchestrator {
    config: OrchestratorConfig,
    graph_store: Arc<dyn GraphStore>,
    message_bus: Arc<dyn MessageBus>,
    active_workers: HashMap<AgentId, WorkerHandle>,
    file_locks: FileOwnershipMap,  // Which agent owns which files
}

pub struct OrchestratorConfig {
    pub max_concurrent_workers: usize,  // Default: 4
    pub max_retries_per_task: usize,    // Default: 2
    pub worker_turn_limit: usize,       // Default: 100
    pub check_in_interval: usize,       // Worker reports every N turns
    pub review_required: bool,          // Spawn reviewer after coder finishes

    // Error handling thresholds
    pub max_consecutive_llm_failures: usize,    // Default: 3. Persistent failures → TaskBlocked
    pub max_consecutive_tool_failures: usize,   // Default: 3. Bad tool calls → TaskBlocked
    pub worker_token_budget: usize,             // Default: 200_000. Per-worker token limit
    pub token_budget_warning_pct: u8,           // Default: 80. Inject "wrap up" at this %
    pub max_tokens_per_goal: Option<usize>,     // Optional. Pause + approval request if exceeded
}
```

### Orchestrator State Machine

```
┌───────────┐
│  Startup  │  Load config, connect DB, recover interrupted session
└─────┬─────┘
      │
      ▼
┌───────────┐  no goal found
│  Loading   │──────────────→ Create new goal from user input
└─────┬─────┘
      │ goal exists (possibly from previous session)
      ▼
┌───────────┐  no tasks yet
│ Planning   │──────────────→ Spawn planner worker
└─────┬─────┘
      │ tasks exist
      ▼
┌───────────┐
│ Scheduling │  Query ready tasks, group into work packages,
└─────┬─────┘  check file ownership constraints, spawn workers
      │
      ▼
┌───────────┐  workers running
│ Monitoring │  Wait for worker messages (progress, completion, blocks)
└─────┬─────┘  Handle timeouts, failures, new task creation
      │
      ├──→ Worker completed → update task, back to Scheduling
      ├──→ Worker blocked → record reason, back to Scheduling
      ├──→ Worker failed → retry or mark failed, back to Scheduling
      ├──→ All tasks complete → to Reviewing
      └──→ Session interrupted → generate handoff notes, persist state

      ▼
┌───────────┐
│ Reviewing  │  (if review_required) Spawn reviewer workers
└─────┬─────┘
      │
      ▼
┌───────────┐
│ Completing │  Generate session summary, store observations,
└───────────┘  report results to user
```

### Work Packages

Instead of assigning one task to one worker (wasteful if tasks are small), the orchestrator groups related tasks into **work packages**:

```rust
pub struct WorkPackage {
    pub id: String,
    pub task_ids: Vec<String>,
    pub file_scope: Vec<PathBuf>,      // Files this package touches
    pub profile: String,                // Which agent profile to use
    pub priority: Priority,
    pub estimated_complexity: Complexity,
}

pub enum Complexity {
    Small,   // 1-2 file changes, straightforward
    Medium,  // Multiple files, some decision-making
    Large,   // Architectural changes, many files
}
```

**Grouping rules:**
1. Tasks that modify the same files → same work package (prevents conflicts)
2. Tasks with sequential dependencies → same work package (one worker handles the chain)
3. Independent tasks with separate file scopes → separate work packages (parallel execution)
4. A single large task → its own work package

### File Ownership

To prevent two workers from modifying the same file simultaneously:

```rust
pub struct FileOwnershipMap {
    locks: HashMap<PathBuf, AgentId>,
}

impl FileOwnershipMap {
    /// Try to acquire ownership of files for an agent.
    /// Returns Err if any file is already owned by another agent.
    pub fn acquire(&mut self, agent_id: &AgentId, files: &[PathBuf]) -> Result<()>;

    /// Release all files owned by an agent (when worker completes).
    pub fn release(&mut self, agent_id: &AgentId);

    /// Check if a file write is permitted for an agent.
    pub fn can_write(&self, agent_id: &AgentId, file: &Path) -> bool;
}
```

The security layer checks file ownership before allowing writes. Workers declare their file scope when spawned; the orchestrator validates no overlaps before starting the worker.

### Worker Lifecycle

```rust
pub struct WorkerHandle {
    pub id: AgentId,
    pub profile: String,
    pub work_package: WorkPackage,
    pub state: WorkerState,
    pub join_handle: JoinHandle<Result<AgentOutcome>>,
    pub cancel_token: CancellationToken,
    pub spawned_at: DateTime<Utc>,
    pub last_check_in: DateTime<Utc>,
}

pub enum WorkerState {
    Spawning,         // Tokio task created, agent initializing
    Initializing,     // Loading context (AGENTS.md, graph nodes, task details)
    Working,          // Active LLM loop (calling tools, writing code)
    Reporting,        // Generating completion report
    Completed(AgentOutcome),
    Failed(String),
}
```

Workers are tokio tasks. Each worker:
1. Receives an `AgentContext` with its work package, relevant observations, decisions, and AGENTS.md context
2. Runs the standard agentic loop (LLM call → tool execution → repeat)
3. Reports progress every N turns to the orchestrator via the message bus
4. Signals completion or blocking via the signal tool
5. Gets terminated (cancel token) if it exceeds turn limits or the session ends

### Error Handling Strategy

Errors are handled at two levels: the **AgentRuntime** (within a worker) and the **Orchestrator** (across workers).

**Within a worker (AgentRuntime):**

- **Transient LLM failures** (network errors, rate limits): Handled by the existing `llm/retry.rs` retry logic with exponential backoff. A single failed API call does not kill the worker.
- **Persistent LLM failures** (`max_consecutive_llm_failures` consecutive failures, default 3): Worker gives up and signals `TaskBlocked` with the error details. The orchestrator marks the task Blocked.
- **Malformed tool calls**: AgentRuntime catches these and sends the error back to the LLM as a tool result (e.g., "invalid tool call: unknown tool 'foo'"). The LLM gets a chance to self-correct. Counts toward a "confusion counter" — after `max_consecutive_tool_failures` consecutive bad calls (default 3), worker signals blocked.
- **Invalid graph operations**: GraphStore validates inputs and returns errors. AgentRuntime feeds the error back to the LLM as a tool result. Normal self-correction applies.
- **Token budget**: AgentRuntime tracks cumulative tokens (input + output) per worker. At `token_budget_warning_pct` (default 80%), a system message is injected: "You are approaching your token budget. Wrap up your current work and signal completion with what you've accomplished." At 100% of `worker_token_budget` (default 200,000), the worker is force-stopped and signals incomplete — the orchestrator treats this as a partial completion, not a failure.

**Across workers (Orchestrator):**

- **Task retry semantics**: When a task fails or a worker is blocked, the orchestrator checks the retry count against `max_retries_per_task` (default 2). If retries remain: task status resets to Ready, a fresh worker is spawned with fresh context. The previous attempt's Outcome node (with `success: false`) is visible in the graph, so the new worker can learn from it.
- **Exhausted retries**: Task is marked Failed. The orchestrator creates an Observation node documenting the failure pattern and continues with other tasks. If the failed task blocks downstream work, those tasks become Blocked with reason referencing the failed task.
- **Worker crash** (panic, OOM): The tokio task's `JoinHandle` returns an error. Orchestrator treats this identically to a persistent failure — resets task to Ready if retries remain.

### Token Accounting

Each worker tracks cumulative token usage (input + output) during execution. On completion, the total is reported to the orchestrator and stored on the session record.

**Goal-level budget**: If `max_tokens_per_goal` is set, the orchestrator checks cumulative usage across all workers for that goal before spawning new workers. If the budget is exceeded, the orchestrator pauses and surfaces an approval request — regardless of autonomy level. The user can approve continued spending, adjust the budget, or stop the goal.

**Visibility**: `rustagent status` shows cumulative tokens for the active goal. Session records include total tokens consumed. No dollar-cost calculation — token counts are provider-agnostic, and pricing changes too frequently to maintain a rate table. Users multiply by their provider's rate.

### Communication Model

**Two channels, clearly separated:**

1. **SQLite (source of truth)**: All durable state — graph nodes, edges, task status, observations, outcomes. Workers write here; the orchestrator reads here. If anything crashes, the DB has the complete picture.
2. **Message bus (real-time signals)**: In-memory, fire-and-forget notifications. Used to wake the orchestrator immediately rather than waiting for it to poll the DB.

The message bus has **no durability guarantees**. Messages are best-effort, in-memory only. If a message is lost (worker cancelled, bus drops it), no state is corrupted — the orchestrator discovers the same information by querying the DB on its next scheduling pass. This means the system is correct even if the message bus fails completely; it's just slower.

**Message directions:**
- Worker → Orchestrator: "I'm done", "I'm blocked", "I need a scope change"
- Orchestrator → Worker: "Cancel", "Here's additional context"
- Worker ↔ Worker: Only for review workflows (reviewer sends feedback to coder)
- Workers never message other workers directly outside of review flow

```rust
pub enum WorkerMessage {
    // Worker → Orchestrator
    ProgressReport { turn: usize, summary: String },
    TaskCompleted { task_id: String, summary: String },
    TaskBlocked { task_id: String, reason: String },
    NeedsDecision { task_id: String, decision: GraphNode },
    NodeCreated { parent_id: String, node: GraphNode },

    // Orchestrator → Worker
    Cancel { reason: String },
    AdditionalContext { content: String },

    // Worker ↔ Worker (review flow)
    ReviewRequest { work_package_id: String, changed_files: Vec<String> },
    ReviewFeedback { approved: bool, comments: Vec<String> },
}
```

### Recovery

**Every piece of state is in SQLite.** Recovery is straightforward:

1. Orchestrator starts → checks for interrupted session
2. Loads goal, node tree, and last session's handoff notes
3. Task nodes that were InProgress when the crash happened → reset to Ready
4. All graph nodes from the interrupted session are preserved
5. Resume from Scheduling state

Workers that die mid-execution: their tasks are reset to Ready and will be reassigned. Any files they partially modified are detectable via `git diff` and can be rolled back if needed.

### Keeping Workers On Task

1. **Focused context**: ContextBuilder gives each worker only relevant info (task details, related observations, applicable AGENTS.md sections, relevant past decisions)
2. **Acceptance criteria**: Workers check their own work against criteria before signaling completion
3. **File scope enforcement**: Security layer prevents writes outside the work package's declared file scope
4. **Turn limits**: Workers have a configurable max turn count; if exceeded, they must report what they accomplished and what's remaining
5. **Check-in intervals**: Every N turns, workers send a progress summary to the orchestrator
6. **Decision logging**: For significant choices, workers must log a decision node before proceeding (encouraged via system prompt, not enforced)

### Dynamic Adaptation

The deterministic orchestrator cannot make judgment calls, but it handles scope changes and emergent work through defined mechanisms:

**File scope expansion**: If a worker needs files outside its declared scope, the security layer blocks the write. The worker signals `NeedsDecision` with the requested files. The orchestrator checks for conflicts with other active workers — if no conflict, it expands the scope and sends `AdditionalContext` to the worker to resume. If there's a conflict, the task is re-queued for after the conflicting worker finishes.

**Task splitting**: Workers can create subtask nodes under their current task via `create_node`. When the worker completes, the orchestrator discovers the new nodes and schedules them through the normal Scheduling flow. This handles "this task turned out to be three tasks" without requiring re-planning.

**Re-planning trigger**: When workers create more than `re_plan_threshold` new tasks during a session (configurable, default 5), or when a worker signals `NeedsDecision` about overall approach (not just file scope), the orchestrator spawns a fresh planner worker to reassess the full task tree. The planner sees the current graph state — completed tasks, new subtasks, observations — and can reorganize remaining work.

### Scaling Considerations

The orchestrator can handle many workers because:
- Workers are independent tokio tasks with no shared mutable state
- All coordination goes through SQLite (WAL mode for concurrent reads)
- File ownership map is the only shared in-process state (behind a mutex, very fast operations)
- Message bus is fire-and-forget for monitoring (no back-pressure issues)

Config option `max_concurrent_workers` controls parallelism. Default 4, but can be increased for larger projects with well-separated tasks.

---

## Observability

### Structured Logging

The existing `src/logging.rs` (tracing + daily rotation to `~/.local/state/rustagent/logs/`) is carried forward and extended. All log spans are tagged with `agent_id`, `goal_id`, and `task_id` where applicable, so logs can be filtered per worker after the fact.

### Worker Conversation Persistence

The full LLM conversation for each worker (all messages, tool calls, and tool results) is stored in a dedicated table. This is the single most useful debugging artifact — it allows replaying exactly what an agent saw and did.

```sql
CREATE TABLE worker_conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    agent_id TEXT NOT NULL,
    task_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array of task IDs in the work package
    messages TEXT NOT NULL,               -- JSON array of the full conversation
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    completed_at TEXT
);
```

Conversations are written on worker completion (or failure). They are not streamed incrementally — the full conversation is available after the worker finishes.

### What's Deferred

- **Metrics collection** (task completion rate, agent utilization, retry rates): Not for v2. Premature until we know what to measure from real usage.
- **Alerting**: Not for v2. Failures are surfaced through WebSocket events and the CLI.

---

## Git Integration

### No Dedicated Git Tool

Agents interact with git through the shell tool (bash), not a dedicated `git.rs` tool. This avoids constraining agents to a limited git API — they can run any git command their workflow requires, and users don't hit friction where the dedicated tool doesn't cover their specific needs. Git command access is controlled through the profile's `allowed_commands` patterns (e.g., `"git *"`).

### Worktree-Based Parallel Isolation

Parallel workers operate in separate **git worktrees**, providing true filesystem isolation rather than relying solely on the file ownership map as policy enforcement.

```
project/                              # Main worktree (user's working directory, untouched)
.git/worktrees/
├── ra-a3f8-wp-001/                  # Worktree for work package 001
├── ra-a3f8-wp-002/                  # Worktree for work package 002
└── ra-a3f8-wp-003/                  # Worktree for work package 003
```

**Lifecycle:**

1. **Goal start**: Orchestrator creates a goal branch (`rustagent/ra-a3f8`) from the current HEAD
2. **Work package spawn**: For each work package, the orchestrator creates a worktree branching from the goal branch (`rustagent/ra-a3f8/wp-001`). The worker's `AgentContext` includes the worktree path — all file operations are rooted there.
3. **Worker execution**: Each worker reads and writes files in its own worktree. Git commands (status, diff, add, commit) operate on the worktree's branch. Workers commit their changes on completion as part of the Reporting state, with a conventional message referencing task IDs.
4. **Work package merge**: On worker completion, the orchestrator merges the work package branch into the goal branch. Since work packages have non-overlapping file scopes (enforced by the file ownership map), merges are always clean.
5. **Goal completion**: All work package branches are merged. The goal branch contains the combined work. The user decides whether to merge into their main branch, push, or take other action.
6. **Cleanup**: Worktrees and work package branches are removed after successful merge into the goal branch.

### Commit Strategy

- One commit per work package completion, with message: `rustagent: <task titles> (ra-a3f8.1, ra-a3f8.2)`
- In Supervised/Gated mode, the PreCommit approval gate fires before the commit is finalized
- In Full autonomy mode, agents can also push if the workflow requires it

### Why Worktrees

- **True isolation**: Workers can't accidentally read or write each other's uncommitted changes
- **No merge conflicts between workers**: Non-overlapping file scopes + separate branches = always-clean merges
- **User's working directory is untouched**: The main worktree stays clean — agents work in their own space
- **Native git**: No custom locking or coordination — git handles everything

---

## Autonomy Levels

### Levels

Three autonomy levels control how much human oversight the orchestrator requires:

```rust
// src/config/autonomy.rs

pub enum AutonomyLevel {
    Full,        // No gates. Orchestrator runs to completion autonomously.
    Supervised,  // Gates at key milestones. User reviews and can redirect.
    Gated,       // Gates at every state transition. User approves each step.
}
```

### Approval Gates

```rust
pub enum ApprovalGate {
    PlanReview,       // After planner creates task tree, before execution begins
    PreCommit,        // Before code changes are committed to git
    TaskComplete,     // Before a task is marked Complete (review the work)
    DecisionPoint,    // When an agent logs a Decision with multiple options
    GoalComplete,     // Before the goal is marked Complete (final review)
    WorkerSpawn,      // Before each new worker is spawned
}
```

**Which gates are active per level:**

| Gate | Full | Supervised | Gated |
|------|------|------------|-------|
| PlanReview | - | Yes | Yes |
| PreCommit | - | Yes | Yes |
| TaskComplete | - | Yes | Yes |
| DecisionPoint | - | - | Yes |
| GoalComplete | - | Yes | Yes |
| WorkerSpawn | - | - | Yes |

### Orchestrator Integration

When the orchestrator hits an active gate:

1. Emits an `ApprovalRequest` event (includes gate type, context summary, proposed action)
2. Pauses the relevant operation (but other workers on unrelated tasks continue)
3. Waits for a response: `Approve`, `Reject(reason)`, or `Modify(instructions)`

- **Approve**: Orchestrator proceeds as planned
- **Reject**: Operation is cancelled. For PlanReview, planner is re-spawned with rejection feedback. For TaskComplete, task returns to InProgress. For PreCommit, changes are kept but not committed.
- **Modify**: Orchestrator incorporates the instructions. For PlanReview, planner is re-spawned with modification guidance. For DecisionPoint, the user's choice is recorded as the chosen option.

### How Approval Requests Reach the User

- **CLI mode**: Orchestrator blocks and prints the approval request to stdout. User responds interactively.
- **Daemon mode**: Approval request is emitted as a WebSocket event and stored in a pending approvals table. Web UI shows a notification. CLI can also poll: `rustagent approvals` lists pending, `rustagent approve <id>` / `rustagent reject <id> --reason "..."` responds.

### Scoping

- **Default**: Set per-project in project config (`.rustagent/config.toml` or at registration time)
- **Override**: Per-goal at run time: `rustagent run --autonomy supervised "Add auth"`
- **Fallback**: If no project default is set, defaults to `Supervised`

---

## Module Structure (Updated)

```
src/
├── main.rs                       # CLI: project, run, plan, status, tasks, sessions, decisions, graph (import/export/diff), search, daemon
├── lib.rs
├── config/
│   ├── mod.rs                    # Config loading + env var expansion
│   ├── agents.rs                 # Agent profile parsing
│   └── autonomy.rs               # AutonomyLevel, ApprovalGate types
├── project.rs                    # Project type, registration, cwd resolution
├── logging.rs                    # Carry forward
├── llm/                          # Carry forward entirely
│   ├── mod.rs, anthropic.rs, openai.rs, ollama.rs, mock.rs
│   ├── error.rs, retry.rs, factory.rs
├── agent/
│   ├── mod.rs                    # Agent trait, AgentId, AgentContext, AgentOutcome, WorkerState
│   ├── profile.rs                # AgentProfile (role, prompt, tools, LLM config, security scope)
│   ├── runtime.rs                # AgentRuntime - generic agentic loop (the worker's brain)
│   ├── orchestrator.rs           # Deterministic state machine: scheduling, monitoring, recovery
│   ├── work_package.rs           # WorkPackage grouping + file ownership map
│   └── builtin_profiles.rs       # Default planner/coder/reviewer/tester/researcher
├── message/
│   ├── mod.rs                    # Envelope, AgentMessage, MessageBus trait
│   ├── bus.rs                    # TokioMessageBus
│   └── envelope.rs               # Envelope type
├── graph/
│   ├── mod.rs                    # GraphNode, GraphEdge, NodeType, EdgeType, NodeStatus, Priority
│   ├── store.rs                  # GraphStore trait + SQLite impl
│   ├── query.rs                  # Query builders (node queries, edge traversal)
│   ├── dependency.rs             # Dependency resolution + ready surfacing for task nodes
│   ├── session.rs                # Session management + handoff notes
│   ├── decay.rs                  # Node compaction for context injection (age-based detail levels)
│   ├── export.rs                 # Export decisions to markdown ADR files
│   └── interchange.rs            # TOML import/export for collaboration (graph ↔ .rustagent/graph/)
├── context/
│   ├── mod.rs                    # ContextBuilder (compact structured format + on-demand expansion)
│   └── agents_md.rs              # AGENTS.md parser + resolver (per https://agents.md/ spec)
├── security/
│   ├── mod.rs                    # SecurityValidator (carry forward)
│   ├── permission.rs             # PermissionHandler (carry forward)
│   └── scope.rs                  # Per-agent SecurityScope
├── tools/
│   ├── mod.rs                    # Tool trait + ToolRegistry (carry forward)
│   ├── factory.rs                # Extended factory
│   ├── file.rs, shell.rs, signal.rs, permission_check.rs  # Carry forward
│   ├── search.rs                 # Code search
│   ├── graph_tools.rs            # Unified: create_node, update_node, add_edge, claim_task, log_decision, choose_option, record_outcome, query_nodes, search_nodes
│   └── agent_tools.rs            # spawn_sub_agent, send_message, query_agent_status
├── daemon/
│   ├── mod.rs                    # Daemon startup, shutdown, PID management
│   ├── server.rs                 # Axum HTTP server setup + routes
│   ├── api/
│   │   ├── mod.rs                # API route handlers
│   │   ├── projects.rs           # Project CRUD endpoints
│   │   ├── graph.rs              # Node/edge CRUD, task views, decision views
│   │   ├── search.rs             # Full-text search endpoint
│   │   └── agents.rs             # Agent status endpoints
│   └── ws.rs                     # WebSocket handler + event broadcasting
├── db/
│   ├── mod.rs                    # Database init + connection pool
│   └── migrations.rs             # Schema (projects, nodes, edges, sessions, nodes_fts, worker_conversations)
```

## Agent Profiles

### AgentProfile Type

```rust
// src/agent/profile.rs

pub struct AgentProfile {
    pub name: String,                      // e.g., "coder", "planner", "my-rust-coder"
    pub extends: Option<String>,           // Built-in to inherit from
    pub role: String,                       // One-line role description
    pub system_prompt: String,             // Full system prompt (or template with {{variables}})
    pub allowed_tools: Vec<String>,        // Tool group names: "file", "shell", "graph", "signal", "search", "agent"
    pub security: SecurityScope,
    pub llm: ProfileLlmConfig,
    pub turn_limit: Option<usize>,         // Override OrchestratorConfig.worker_turn_limit
    pub token_budget: Option<usize>,       // Override OrchestratorConfig.worker_token_budget
}

pub struct SecurityScope {
    pub allowed_paths: Vec<String>,        // Glob patterns for file access (e.g., "src/**", "tests/**")
    pub denied_paths: Vec<String>,         // Explicit denials (e.g., ".env", "**/*.key")
    pub allowed_commands: Vec<String>,     // Shell command patterns (e.g., "cargo *", "git diff *")
    pub read_only: bool,                   // If true, file writes are blocked regardless of path
    pub can_create_files: bool,            // Can create new files (vs only editing existing)
    pub network_access: bool,              // Can make outbound network requests via tools
}

pub struct ProfileLlmConfig {
    pub model: Option<String>,             // Override default model (e.g., use cheaper model for planning)
    pub temperature: Option<f64>,
    pub max_tokens: Option<usize>,         // Per-response max tokens
}
```

### Built-in Profiles

Five built-in profiles cover the standard workflow. Custom profiles can extend these.

| Profile | Role | Tools | File Access | Key Behavior |
|---------|------|-------|-------------|--------------|
| **planner** | Breaks goals into tasks and decisions | graph, signal | Read-only | Creates task tree, decision nodes, dependency edges. No code changes. |
| **coder** | Implements tasks by writing code | file, shell, graph, signal | Write (scoped to work package) | Executes tasks, logs decisions for non-trivial choices, records outcomes. Git via shell. |
| **reviewer** | Reviews completed work for correctness | file, shell, graph, signal | Read-only | Reads code, runs tests/lints, creates Observation nodes for issues found. |
| **tester** | Writes and runs tests | file, shell, graph, signal | Write (scoped to test dirs) | Writes tests against acceptance criteria, runs them, reports coverage. |
| **researcher** | Gathers information and context | file, shell, search, graph, signal | Read-only | Reads code, searches, stores findings as observations. |

**System prompt structure** for all built-in profiles follows a common template:

```
You are a {role} agent working on project "{project_name}".

## Your Task
{task_description}

## Acceptance Criteria
{acceptance_criteria}

## Context
{relevant_decisions}
{agents_md_content}

## Rules
- {profile-specific rules}
- When you make a non-trivial choice between alternatives, log a decision using log_decision.
- When you discover something noteworthy, record it using record_observation.
- Signal completion or blocking using the signal tool. Do not simply stop.
```

Profile-specific rules (examples):
- **planner**: "Break work into tasks that can be completed independently. Keep tasks small enough for a single focused session. Specify acceptance criteria for every task."
- **coder**: "Check your work against the acceptance criteria before signaling completion. Only modify files within your declared scope. Commit logical units of work."
- **reviewer**: "Do not modify files. Report issues as Observation nodes. Approve or reject via the signal tool with specific feedback."

### Custom Profiles (TOML)

Custom profiles live in two locations, both using one TOML file per profile:

```
# Project-level (version-controlled, shared with team)
.rustagent/
├── profiles/
│   ├── rust-coder.toml
│   └── docs-writer.toml
├── config.toml               # Project config (autonomy level, defaults)
├── graph/                    # Graph export (already specified)
│   └── ...
└── sessions/
    └── ...

# User-level (personal preferences, not version-controlled)
~/.config/rustagent/profiles/
├── my-coder.toml
└── my-planner.toml
```

Example project-level profile:

```toml
# .rustagent/profiles/rust-coder.toml

name = "rust-coder"
extends = "coder"                            # Inherits all coder defaults
role = "Rust implementation specialist"
system_prompt = """
You are a senior Rust developer. Follow these project conventions:
- Use thiserror for error types, anyhow in binaries
- Prefer &str over String in function parameters
- Write doc comments for all public items
"""

allowed_tools = ["file", "shell", "graph", "signal"]

[security]
allowed_paths = ["src/**", "tests/**", "Cargo.toml", "Cargo.lock"]
denied_paths = [".env", "**/*.key", "**/*.pem"]
allowed_commands = ["cargo *", "rustfmt *", "git diff *", "git status"]
read_only = false
can_create_files = true
network_access = false

[llm]
model = "claude-sonnet-4-20250514"
temperature = 0.3
```

### Inheritance Rules

When a profile specifies `extends`:

1. **All fields start as copies of the parent profile**
2. **Scalar fields** (role, system_prompt, read_only, etc.): child value replaces parent if specified
3. **List fields** (allowed_tools, allowed_paths, allowed_commands): child value **replaces** parent entirely (not merged). This prevents accidentally widening access by inheriting a broad parent and adding more.
4. **system_prompt**: If the child specifies a system_prompt, it is **appended** to the parent's prompt (separated by a newline section header `## Project-Specific Instructions`). This preserves the structural template while allowing customization. To fully replace, set `extends` to null.
5. **Unset optional fields** (turn_limit, token_budget, llm overrides): fall through to parent, then to orchestrator defaults.

### Profile Resolution

When the orchestrator spawns a worker, profiles are resolved in priority order:

1. **Project-level**: `.rustagent/profiles/*.toml` in the project root (checked into git)
2. **User-level**: `~/.config/rustagent/profiles/*.toml` (personal overrides)
3. **Built-in profiles**: Compiled-in defaults (planner, coder, reviewer, tester, researcher)

First match wins. If the resolved profile has `extends`, the parent is resolved through the same chain and inheritance rules are applied.

**Validation** (at resolution time):
- `allowed_tools` only references known tool groups
- `allowed_paths` doesn't escape the project root
- `extends` doesn't create cycles
- Profile names are unique within each level (duplicate across levels is fine — higher priority wins)

---

## Context Assembly

### ContextBuilder

The `ContextBuilder` assembles an `AgentContext` for each worker at spawn time. It uses two strategies to minimize token consumption while keeping all context accessible:

- **Compact structured format** for essential context (task details, decisions, handoff notes) — always included inline
- **Summary + on-demand expansion** for reference context (observations, AGENTS.md) — titles/one-liners inline, full detail available via tools

### Context Template

The assembled context is injected as the system prompt. Target: **3-4K tokens** for priorities 1-7, leaving the rest of the context window for the agentic loop.

```
## Role
{profile.role}

## Task
[TASK] {task.id} | {task.title} | priority={task.priority}
[CRITERIA] {acceptance_criteria, semicolon-separated}
[DEP:DONE] {completed_dependency_id} → {title} (completed)
[DEP:PENDING] {pending_dependency_id} → {title} (status)

## Previous Attempt (if retry)
[PREV_ATTEMPT] {outcome.description}

## Session Continuity
[HANDOFF] {handoff_notes, compressed to key facts}

## Active Decisions
[DECISION] {id} | {title} | chosen: {chosen_option} | reason: {rationale}

## Relevant Observations (use query_nodes(id) for full detail)
- {node_id}: {one-line summary}
- {node_id}: {one-line summary}

## Project Conventions (use read_agents_md(path) for full text)
- {path}: {section_title} ({rule_count} rules)
- {path}: {section_title} ({rule_count} rules)

## Rules
{profile.system_prompt rules section}
```

### Token Budget Allocation

| Priority | Content | Strategy | Budget |
|----------|---------|----------|--------|
| 1 (required) | System prompt (profile template + rules) | Compact inline | ~800 tokens |
| 2 (required) | Task details + acceptance criteria | Compact structured | ~400 tokens |
| 3 (high) | Previous attempt outcomes (if retry) | Compact structured | ~300 tokens |
| 4 (high) | Handoff notes from last session | Compact structured | ~300 tokens |
| 5 (medium) | Active decisions relevant to this task | Compact structured | ~400 tokens |
| 6 (medium) | Relevant observations | Summary only; `query_nodes(id)` for detail | ~300 tokens |
| 7 (medium) | AGENTS.md sections | Summary only; `read_agents_md(path)` for detail | ~200 tokens |
| 8 (remainder) | Conversation history (agentic loop) | Grows during execution | Everything else |

**Overflow handling**: If the total for priorities 1-7 exceeds the budget (e.g., many decisions, many observations), items are trimmed from the bottom of each section (least relevant first, determined by recency and graph distance from the current task).

### On-Demand Expansion Tools

Workers can pull full detail during execution using existing tools:

- `query_nodes(node_id)` → Returns full node detail with edges (in graph_tools)
- `search_nodes(query)` → Full-text search over all node titles/descriptions (in graph_tools)
- `read_agents_md(path)` → Returns full AGENTS.md content for a directory path (in context module)

This means workers aren't missing anything — they just have to ask for it. The initial context tells them *what exists* so they know what to ask for.

### Static Context

Context is built once at worker spawn and does not change during execution. If a worker needs fresh information mid-execution (e.g., checking if a dependency was completed by another worker), it uses `query_nodes` to read current graph state. This avoids the confusion of context shifting under the agent.

### AGENTS.md Support

Rustagent follows the [AGENTS.md specification](https://agents.md/) — a simple, open format for guiding coding agents adopted by 60,000+ open-source projects. AGENTS.md files are free-form Markdown with no required sections or special syntax. They serve as project-specific instructions for agents (build commands, code style, testing conventions, security rules, etc.).

**Scoping**: Per the spec, "the closest AGENTS.md to the edited file wins." Rustagent resolves this by walking the directory hierarchy toward the worker's file scope:

1. Start at project root, collect `AGENTS.md` if present
2. Walk toward each path in the work package's file scope
3. Collect `AGENTS.md` at each intermediate directory
4. The closest file to the target takes precedence; root-level `AGENTS.md` is always included as baseline context

For a worker scoped to `src/auth/**`:
- `AGENTS.md` (project root) → general project rules
- `src/AGENTS.md` → source conventions (if exists)
- `src/auth/AGENTS.md` → auth-specific rules (if exists, takes precedence for auth-related guidance)

**Context injection**: Only file paths and top-level heading summaries are included in the initial compact context. The agent uses `read_agents_md("src/auth")` to retrieve full content on demand.

**Implementation note**: The parser (`src/context/agents_md.rs`) should handle the format as plain Markdown text — no semantic parsing of sections beyond extracting headings for the summary. Explicit user prompts (the task description and acceptance criteria) override AGENTS.md instructions per the spec's conventions.

### Node Decay Thresholds

Completed graph nodes are compacted based on age for context injection. These thresholds are configurable in `.rustagent/config.toml`:

| Age | Detail Level | Example |
|-----|-------------|---------|
| Recent (< 7 days) | Full: description, criteria, outcomes | "Implemented JWT validation using jsonwebtoken crate. Tokens validated against RS256 keys from JWKS endpoint." |
| Older (7-30 days) | Summary: title, status, key outcome | "JWT validation — completed, success" |
| Ancient (> 30 days) | Minimal: title, status | "JWT validation — completed" |

```toml
# .rustagent/config.toml
[context.decay]
recent_days = 7
older_days = 30
# Nodes older than older_days get minimal detail
```

Decay applies when nodes are included in context assembly. Full detail is always available via `query_nodes` regardless of age.

---

## Core Traits

### Agent
```rust
#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &AgentId;
    fn profile(&self) -> &AgentProfile;
    async fn run(&self, ctx: AgentContext) -> Result<AgentOutcome>;
    fn cancel(&self);
}
```

### GraphStore
```rust
#[async_trait]
pub trait GraphStore: Send + Sync {
    // Node operations
    async fn create_node(&self, node: &GraphNode) -> Result<()>;
    async fn update_node(&self, node: &GraphNode) -> Result<()>;
    async fn get_node(&self, id: &str) -> Result<Option<GraphNode>>;
    async fn query_nodes(&self, query: NodeQuery) -> Result<Vec<GraphNode>>;

    // Task-specific convenience methods
    async fn claim_task(&self, node_id: &str, agent_id: &AgentId) -> Result<bool>; // Atomic claim
    async fn get_ready_tasks(&self, goal_id: &str) -> Result<Vec<GraphNode>>;
    async fn get_next_task(&self, goal_id: &str) -> Result<Option<GraphNode>>; // Priority-based

    // Edge operations
    async fn add_edge(&self, edge: &GraphEdge) -> Result<()>;
    async fn remove_edge(&self, edge_id: &str) -> Result<()>;
    async fn get_edges(&self, node_id: &str, direction: EdgeDirection) -> Result<Vec<(GraphEdge, GraphNode)>>;

    // Graph queries
    async fn get_children(&self, node_id: &str) -> Result<Vec<(GraphNode, EdgeType)>>;
    async fn get_subtree(&self, node_id: &str) -> Result<Vec<GraphNode>>;
    async fn get_active_decisions(&self, project_id: &str) -> Result<Vec<GraphNode>>; // Now mode
    async fn get_full_graph(&self, goal_id: &str) -> Result<WorkGraph>; // History mode
    async fn search_nodes(&self, query: &str, project_id: Option<&str>, limit: usize) -> Result<Vec<GraphNode>>; // FTS5 full-text search

    // Session management
    async fn create_session(&self, session: &Session) -> Result<()>;
    async fn end_session(&self, session_id: &str, handoff_notes: &str) -> Result<()>;
    async fn get_latest_session(&self, goal_id: &str) -> Result<Option<Session>>;
}

pub enum EdgeDirection {
    Outgoing,  // Edges where this node is from_node
    Incoming,  // Edges where this node is to_node
    Both,
}
```

### MessageBus
```rust
#[async_trait]
pub trait MessageBus: Send + Sync {
    async fn send(&self, to: &AgentId, msg: WorkerMessage) -> Result<()>;
    async fn broadcast(&self, msg: WorkerMessage) -> Result<()>;
    fn subscribe(&self, agent_id: &AgentId) -> mpsc::Receiver<WorkerMessage>;
}
```

See [Communication Model](#communication-model) for `WorkerMessage` variants and delivery semantics.

---

## Database Schema

### Schema Versioning and Migrations

```sql
-- Schema version tracking (single row)
CREATE TABLE schema_version (
    version INTEGER NOT NULL,
    migrated_at TEXT NOT NULL
);
```

At startup, the binary checks `schema_version.version` against its expected version:
- **Match**: Proceed normally
- **DB is older**: Run sequential migrations forward (e.g., `migrate_001_to_002`, `migrate_002_to_003`). Each migration is a SQL function in `src/db/migrations.rs`. No external migration tooling — hand-rolled, simple, single-binary friendly.
- **DB is newer**: Error with "your database was created by a newer version of rustagent, please upgrade"
- **No table**: Fresh database, run full schema creation and set version to current

The binary supports reading schema version N-1 (one version back) for graceful upgrades. Older than that produces a clear upgrade error.

### TOML Export Format Versioning

The `[meta]` section already includes `version = 1`. On format changes:
- Bump the version number
- Import checks the version and applies format-specific parsing
- Old format files produce: "this file uses format v1, run `rustagent graph upgrade` to convert to v2"
- The binary supports reading format version N-1

### Tables

```sql
-- Projects
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    registered_at TEXT NOT NULL,
    config_overrides TEXT,
    metadata TEXT NOT NULL DEFAULT '{}'
);

-- Unified work graph: nodes
-- All entity types (goal, task, decision, option, outcome, observation, revisit)
-- live in one table. Type-specific data goes in metadata (JSON).
CREATE TABLE nodes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    node_type TEXT NOT NULL,          -- goal, task, decision, option, outcome, observation, revisit
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT,                    -- goals and tasks
    assigned_to TEXT,                 -- tasks
    created_by TEXT,
    labels TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    blocked_reason TEXT,
    metadata TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_nodes_project ON nodes(project_id);
CREATE INDEX idx_nodes_type ON nodes(node_type);
CREATE INDEX idx_nodes_status ON nodes(status);

-- Unified work graph: edges
-- All relationships (contains, depends_on, leads_to, chosen, rejected, etc.)
CREATE TABLE edges (
    id TEXT PRIMARY KEY,
    edge_type TEXT NOT NULL,          -- contains, depends_on, leads_to, chosen, rejected, supersedes, informs
    from_node TEXT NOT NULL REFERENCES nodes(id),
    to_node TEXT NOT NULL REFERENCES nodes(id),
    label TEXT,                       -- e.g., rejection reason
    created_at TEXT NOT NULL
);

CREATE INDEX idx_edges_from ON edges(from_node);
CREATE INDEX idx_edges_to ON edges(to_node);
CREATE INDEX idx_edges_type ON edges(edge_type);

-- Sessions (temporal, not part of the graph)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    goal_id TEXT NOT NULL REFERENCES nodes(id),
    started_at TEXT NOT NULL,
    ended_at TEXT,
    handoff_notes TEXT,
    agent_ids TEXT NOT NULL DEFAULT '[]',
    summary TEXT
);

-- Full-text search over graph nodes (titles + descriptions)
CREATE VIRTUAL TABLE nodes_fts USING fts5(
    title,
    description,
    content='nodes',
    content_rowid='rowid'
);

-- Triggers to keep FTS index in sync with nodes table
CREATE TRIGGER nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO nodes_fts(rowid, title, description)
    VALUES (new.rowid, new.title, new.description);
END;
CREATE TRIGGER nodes_ad AFTER DELETE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, title, description)
    VALUES ('delete', old.rowid, old.title, old.description);
END;
CREATE TRIGGER nodes_au AFTER UPDATE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, title, description)
    VALUES ('delete', old.rowid, old.title, old.description);
    INSERT INTO nodes_fts(rowid, title, description)
    VALUES (new.rowid, new.title, new.description);
END;

-- Worker conversation persistence (see Observability section)
CREATE TABLE worker_conversations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    agent_id TEXT NOT NULL,
    task_ids TEXT NOT NULL DEFAULT '[]',  -- JSON array of task IDs in the work package
    messages TEXT NOT NULL,               -- JSON array of the full conversation
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    completed_at TEXT
);
```

---

## Work Graph Tools for Agents

Agents interact with the unified work graph through these tools. Low-level tools operate on raw nodes/edges; high-level tools provide workflow shortcuts.

### Low-Level (generic graph operations)

```
create_node        - Create any node in the graph
                     Params: { node_type, title, description, parent_id?, priority?, metadata? }

update_node        - Update a node's status or metadata
                     Params: { node_id, status?, title?, description?, metadata? }

add_edge           - Create a relationship between two nodes
                     Params: { edge_type, from_node, to_node, label? }

query_nodes        - Search nodes by type, status, project, text
                     Params: { node_type?, status?, project_id?, query?, parent_id? }

search_nodes       - Full-text search over node titles and descriptions (FTS5)
                     Params: { query, project_id?, node_type?, limit? }
```

### High-Level (workflow shortcuts)

```
claim_task         - Atomically claim a Ready task node
                     Params: { node_id }

log_decision       - Shortcut: create Decision + Option nodes in one call
                     Params: { title, description, options: [{ title, description, pros?, cons? }], parent_id? }

choose_option      - Mark an Option as chosen, reject others, update Decision status
                     Params: { decision_id, option_id, rationale }

record_outcome     - Create Outcome node linked to a Task or Option
                     Params: { parent_id, title, description, success: bool }

record_observation - Create Observation node linked to any node
                     Params: { title, description, related_node_id? }

revisit            - Create Revisit from bad Outcome, optionally start new Decision
                     Params: { outcome_id, reason, new_decision_title? }
```

---

## Implementation Phases

### Phase 1a: Database + Projects
1. Set up `src/db/` - Central SQLite at `~/.local/share/rustagent/rustagent.db`, WAL mode, `BEGIN IMMEDIATE` discipline
2. Implement `src/db/migrations.rs` - Full schema (projects, nodes, edges, sessions, nodes_fts, worker_conversations)
3. Implement `src/project.rs` - Project type, registration, resolution from cwd
4. Wire up basic CLI - `project` subcommand (add/list/remove/show)

**Verification:** `rustagent project add test-proj .` registers a project. `rustagent project list` shows it. `rustagent project show test-proj` returns details. Database created at XDG data dir with correct schema.

### Phase 1b: Graph Model + Node Lifecycle
1. Implement `src/graph/mod.rs` - GraphNode, GraphEdge, NodeType, EdgeType, NodeStatus, Priority types
2. Implement `src/graph/store.rs` - GraphStore trait + SQLite implementation (CRUD for nodes and edges)
3. Implement `src/graph/query.rs` - Query builders (node queries, edge traversal, FTS5 search)
4. Implement `src/graph/dependency.rs` - Dependency resolution, ready surfacing, next task recommendation
5. Build `src/tools/graph_tools.rs` - All graph tools (create_node, update_node, add_edge, claim_task, log_decision, choose_option, record_outcome, record_observation, revisit, query_nodes, search_nodes)
6. Wire up CLI - `tasks` (list/ready/next/tree), `decisions` (list/now/history/show), `status`

**Verification:** Create nodes and edges via tests. Dependency resolution correctly surfaces ready tasks. `claim_task` is atomic (concurrent claims test). FTS5 search finds nodes by content. CLI commands display graph state.

### Phase 1c: Sessions + Export + Interchange
1. Implement `src/graph/session.rs` - Session management, template-based handoff notes generation
2. Implement `src/graph/export.rs` - ADR export (decision nodes → markdown files)
3. Implement `src/graph/interchange.rs` - TOML import/export/diff for `.rustagent/graph/`
4. Implement `src/graph/decay.rs` - Node compaction by age for context injection
5. Wire up CLI - `sessions` (list/latest), `decisions export`, `graph` (export/import/diff)

**Verification:** Session creates and ends with accurate handoff notes. `rustagent decisions export` generates readable ADR files. `rustagent graph export` produces TOML; `rustagent graph import` round-trips cleanly. Decay returns appropriate detail levels by node age.

### Phase 1d: Agent Runtime + Single-Agent Execution
1. Cherry-pick `src/llm/`, `src/security/`, `src/tools/` with interface adjustments
2. Define `src/agent/mod.rs` - Agent trait, AgentId, AgentContext, AgentOutcome
3. Build `src/agent/profile.rs` - AgentProfile type, built-in profile definitions, profile resolution (project → user → built-in)
4. Build `src/agent/runtime.rs` - Refactor Ralph loop into generic AgentRuntime with error handling (confusion counter, token budget tracking)
5. Extend `src/config/` - Agent profiles, autonomy mode
6. Update AgentRuntime to encourage decision logging in system prompts
7. Wire up CLI - `run` (single-agent mode), `search`

**Verification:** `rustagent run --project test-proj "goal"` executes with a single agent. Agent creates tasks, makes decisions, records observations. Profile resolution works (project `.rustagent/profiles/` overrides built-ins). Token budget warning triggers at configured threshold.

### Phase 2: Multi-Agent Orchestration
1. Build `src/agent/work_package.rs` - WorkPackage type, file ownership map, grouping logic
2. Build `src/agent/orchestrator.rs` - Full state machine (Startup → Loading → Planning → Scheduling → Monitoring → Reviewing → Completing), recovery logic
3. Build `src/message/` - WorkerMessage types, TokioMessageBus (broadcast + per-agent mpsc)
4. Build `src/agent/builtin_profiles.rs` - System prompts for planner, coder, reviewer, tester, researcher
5. Build `src/tools/agent_tools.rs` - spawn_sub_agent, send_message, query_agent_status
6. Update orchestrator ↔ runtime integration: check-in intervals, turn limits, file scope enforcement
7. Update CLI - `run` uses orchestrator, `status` shows active workers + task progress

**Verification:** `rustagent run "goal"` → orchestrator spawns planner → planner creates tasks → orchestrator groups into work packages → spawns concurrent coder workers for independent packages → workers complete → orchestrator marks tasks done. Recovery test: kill mid-execution, restart, verify it resumes.

### Phase 3: Daemon + HTTP API
1. Build `src/daemon/mod.rs` - Daemon lifecycle (start, stop, PID file)
2. Build `src/daemon/server.rs` - Axum server setup, route mounting, static file serving
3. Build `src/daemon/api/` - All REST endpoints (projects, graph nodes/edges, search, agents)
4. Build `src/daemon/ws.rs` - WebSocket handler + broadcast integration with message bus
5. Update CLI to detect daemon and route commands through API when available
6. Add `daemon` subcommand to main.rs (start, stop, status, logs)

**Verification:** `rustagent daemon start` starts server. `curl localhost:7400/api/projects` returns data. WebSocket connects and receives events during `rustagent run`.

### Phase 4: Web UI
1. Initialize `web/` with Bun + Vite + Svelte 5 + TypeScript
2. Build API client and WebSocket connection handler
3. Build Dashboard view (overview across projects)
4. Build Task Tree view (projection of work graph: task nodes with hierarchy)
5. Build Decision Graph view (projection of work graph: decision/option/outcome nodes, now/history toggle)
6. Build Agent Monitor view (real-time agent activity feed)
7. Build Graph Search view (FTS5 search + filter)
8. Build Session History view
9. Configure Vite proxy for development, static serving for production

**Verification:** `bun run dev` in `web/` shows dashboard. Creating a goal via API updates the UI in real-time via WebSocket.

### Phase 5: AGENTS.md + Context + Polish
1. Build `src/context/agents_md.rs` - Parse AGENTS.md, closest-to-file resolution
2. Build `src/context/mod.rs` - ContextBuilder combining all context sources
3. Build `src/config/autonomy.rs` - Autonomy levels + approval gates
4. Build `src/security/scope.rs` - Per-agent security boundaries
5. Build `src/tools/search.rs`
6. Agent error recovery, task reassignment

**Verification:** AGENTS.md content appears in agent context. Gated mode prompts at configured gates.

---

## Testing Strategy

### Methodology

All implementation follows **test-driven development (TDD)** with strict red-green-refactor:

1. **Red**: Write a failing test that defines the expected behavior
2. **Green**: Write the minimum code to make the test pass
3. **Refactor**: Clean up while keeping tests green

Tests are written *before* implementation, not after. This applies to all phases — graph operations, orchestrator logic, API endpoints, CLI commands, and tools. No feature is considered complete without tests that were written first and observed to fail.

### Test Layers

- **Unit tests**: Pure logic in isolation — graph node lifecycle state machine, dependency resolution, ID generation, TOML serialization round-trips, FTS5 queries, profile inheritance, context assembly, node decay. Use `llm/mock.rs` (carried forward from v1) for LLM interactions.
- **Integration tests**: Components working together — single-agent pipeline (goal → plan → execute → complete), multi-agent orchestration with 2-3 workers, daemon HTTP endpoints (using axum's built-in test utilities), WebSocket event delivery, graph import/export with conflict resolution.
- **Concurrency tests**: Race conditions that matter — multiple workers claiming the same task (exactly one succeeds), concurrent child node creation under one parent (no ID collisions), orchestrator handling simultaneous worker completions.

### What We Don't Test in CI

No real LLM calls in automated tests. Too slow, too expensive, too flaky. Integration tests use the mock LLM client. Real LLM testing is manual, run against a live provider before releases.

---

## New Dependencies

### Rust (Cargo.toml)
```toml
rusqlite = { version = "0.32", features = ["bundled"] }
tokio-rusqlite = "0.6"
tokio-util = "0.7"          # CancellationToken
pulldown-cmark = "0.12"     # AGENTS.md parsing
walkdir = "2.5"             # Directory traversal
glob = "0.3"                # File pattern matching
toml = "0.8"                # TOML serialization for graph interchange
blake3 = "1"                # Content hashing for export change detection
axum = { version = "0.8", features = ["ws"] }           # HTTP server + WebSocket
tower = "0.5"                                             # Middleware
tower-http = { version = "0.6", features = ["cors", "fs"] }  # CORS + static files
rust-embed = { version = "8", features = ["axum"], optional = true }  # Static asset embedding (bundle-ui feature)
```

### Web UI (web/package.json)
```json
{
  "dependencies": {
    "cytoscape": "^3.x"
  },
  "devDependencies": {
    "svelte": "^5.x",
    "@sveltejs/vite-plugin-svelte": "^5.x",
    "typescript": "^5.x",
    "vite": "^6.x"
  }
}
```

## Files to Cherry-Pick from Current Codebase
- `src/llm/*` - All LLM client code
- `src/security/mod.rs` - SecurityValidator
- `src/security/permission.rs` - PermissionHandler trait + impls
- `src/tools/mod.rs` - Tool trait + ToolRegistry
- `src/tools/file.rs` - File operation tools
- `src/tools/shell.rs` - RunCommandTool
- `src/tools/signal.rs` - SignalTool
- `src/tools/permission_check.rs` - FilePermissionChecker
- `src/logging.rs` - Tracing setup
