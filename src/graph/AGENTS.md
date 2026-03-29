# Graph Module

Last verified: 2026-02-09

## Purpose
Provides a persistent, typed work graph for tracking goals, tasks, decisions, and their relationships. Replaces V1's flat JSON spec with a relational model that supports dependency resolution, atomic task claiming, and temporal decay for context injection.

## Contracts
- **Exposes**: `GraphStore` trait (async CRUD for nodes/edges), `SqliteGraphStore` impl, `SessionStore`, `DecayConfig`, TOML interchange (export/import/diff), ADR export
- **Guarantees**: All writes are atomic (BEGIN IMMEDIATE). Task claiming is race-safe. Node status is validated against node type. Child IDs are hierarchical (`parent.seq`). FTS5 index stays in sync via triggers.
- **Expects**: A `Database` instance (from `db` module). Valid `project_id` for nodes. Parent node must exist before creating children.

## Dependencies
- **Uses**: `db::Database`, `chrono`, `blake3` (interchange hashing), `uuid` (ID generation)
- **Used by**: `agent::runtime` (via `Arc<dyn GraphStore>`), `tools::graph_tools`, `main.rs` (CLI commands)
- **Boundary**: Does NOT depend on `llm`, `agent`, or `tools`

## Key Decisions
- SQLite over external DB: Single-file persistence, no daemon, WAL for concurrent reads
- Hierarchical IDs (`ra-XXXX.N.M`): Encode parent-child without extra queries
- Trait-based store: `GraphStore` trait enables test doubles and future backends
- TOML interchange: Git-friendly, deterministic output via BTreeMap, content-hashed for change detection

## Invariants
- Node status must be valid for its NodeType (enforced by `validate_status`)
- Every child node has a Contains edge to its parent (auto-created in `create_node`)
- Completing a task auto-promotes Pending dependents to Ready (inside same transaction)
- Decay levels: Full (<7d), Summary (7-30d), Minimal (>30d) -- configurable via DecayConfig

## Key Files
- `mod.rs` - NodeType (7 variants), EdgeType (7 variants), NodeStatus (15 variants), GraphNode, GraphEdge
- `store.rs` - GraphStore trait (17 methods), SqliteGraphStore, NodeQuery, EdgeDirection, WorkGraph
- `session.rs` - Session, SessionStore, deterministic handoff note generation
- `interchange.rs` - TOML export/import, content hashing, conflict strategies (Skip/Overwrite/Error)
