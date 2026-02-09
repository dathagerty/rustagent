# Rustagent

Last verified: 2026-02-09

## Project Overview

Rustagent is a Rust-based AI agent framework for autonomous task execution. The architecture has two layers:

- **V1 (legacy)**: Planning Agent + Ralph Loop using flat JSON specs
- **V2 (current)**: Graph-based work tracking with typed agent profiles, SQLite persistence, and an agentic runtime loop

V2 is the active development path. V1 modules (`planning/`, `ralph/`, `spec.rs`) remain for backward compatibility.

## Development Commands

```bash
cargo build              # Debug build
cargo build --release    # Optimized release build
cargo check              # Fast compilation check
cargo fmt                # Format code
cargo clippy             # Lint
cargo test               # Run full test suite
cargo test <name>        # Run specific test by name
cargo doc --open         # Generate and view documentation
```

### V2 CLI Commands
```bash
cargo run -- project add <name> <path>   # Register a project
cargo run -- run <goal> --profile coder  # Execute a goal with an agent
cargo run -- tasks                       # List tasks for current project
cargo run -- decisions                   # List decisions
cargo run -- status                      # Show project status
cargo run -- search <query>              # Full-text search graph nodes
cargo run -- sessions                    # View sessions and handoff notes
cargo run -- graph export <goal-id>      # Export graph as TOML
cargo run -- graph import <file>         # Import TOML graph
cargo run -- graph adr <goal-id>         # Export decisions as ADR markdown
```

## Project Structure

```
src/
├── main.rs             # CLI entry point (clap), V1 + V2 commands
├── lib.rs              # Library exports: all public modules
├── config.rs           # Configuration loading with env var substitution
├── logging.rs          # File-based tracing with daily rotation
├── spec.rs             # V1 specification data structures
├── project.rs          # Project type and ProjectStore (CRUD over SQLite)
├── db/                 # Database layer (SQLite + WAL mode)
│   ├── mod.rs          # Database wrapper with async access
│   └── migrations.rs   # Schema versioning and migration framework
├── graph/              # Work graph model (see src/graph/AGENTS.md)
│   ├── mod.rs          # Core types: NodeType, EdgeType, NodeStatus, GraphNode, GraphEdge
│   ├── store.rs        # GraphStore trait + SqliteGraphStore implementation
│   ├── decay.rs        # Node decay for context injection (Full/Summary/Minimal)
│   ├── dependency.rs   # Dependency resolution helpers
│   ├── session.rs      # Session management with handoff notes
│   ├── export.rs       # ADR markdown export
│   └── interchange.rs  # TOML import/export with content hashing
├── agent/              # Agent types and runtime (see src/agent/AGENTS.md)
│   ├── mod.rs          # Agent trait, AgentId, AgentContext, AgentOutcome
│   ├── profile.rs      # AgentProfile with inheritance and SecurityScope
│   ├── builtin_profiles.rs  # 5 built-in profiles: planner, coder, reviewer, tester, researcher
│   └── runtime.rs      # AgentRuntime: agentic loop with token budget and failure thresholds
├── context/            # Context building for agent prompts
│   ├── mod.rs          # ContextBuilder + ReadAgentsMdTool
│   └── agents_md.rs    # AGENTS.md file discovery and heading extraction
├── llm/                # LLM provider abstraction
│   ├── mod.rs          # LlmClient trait, Message, Response (with token tracking)
│   ├── anthropic.rs    # Anthropic (Claude) client
│   ├── openai.rs       # OpenAI client
│   ├── ollama.rs       # Ollama (local models) client
│   ├── mock.rs         # Mock client for testing
│   ├── factory.rs      # Client factory based on config
│   ├── error.rs        # Provider-agnostic error types
│   └── retry.rs        # Rate limit handling with retry logic
├── planning/           # V1 Planning Agent (interactive spec creation)
├── ralph/              # V1 Ralph Loop (spec-based task execution)
├── security/
│   ├── mod.rs          # SecurityValidator for paths/commands
│   ├── permission.rs   # Permission handling (CLI prompts)
│   └── scope.rs        # SecurityScope: per-agent path/command/network restrictions
└── tools/
    ├── mod.rs          # Tool trait and ToolRegistry
    ├── factory.rs      # create_default_registry + create_v2_registry
    ├── graph_tools.rs  # 11 graph tools for agents (create, update, query, claim, etc.)
    ├── file.rs         # read_file, write_file, list_files
    ├── shell.rs        # run_command
    ├── signal.rs       # signal_completion
    └── permission_check.rs  # File permission checking
```

## Key Dependencies

- `rusqlite` (bundled) + `tokio-rusqlite` for async SQLite
- `blake3` for content hashing (interchange format)
- `clap` (derive) for CLI
- `chrono` for timestamps (RFC 3339 everywhere)
- `uuid` v4 for ID generation

## Conventions

### ID Scheme
- Projects/Goals: `ra-XXXX` (4 hex chars from UUID v4)
- Child nodes: `ra-XXXX.N` (dot-separated sequence)
- Edges: `e-XXXXXXXX` (8 hex chars)
- Sessions: `sess-XXXXXXXX`

### Database
- All writes use `BEGIN IMMEDIATE` transactions
- WAL journal mode, foreign keys ON, busy timeout 5000ms
- Schema versioned via `schema_version` table; migrations are forward-only

### Architecture Patterns
- **Factory pattern**: LLM clients (`create_client`), tool registries (`create_v2_registry`)
- **Trait objects**: `dyn LlmClient`, `dyn Tool`, `dyn GraphStore` for runtime polymorphism
- **Arc wrapping**: `Arc<dyn GraphStore>` shared across tools and runtime
- **async_trait**: All async traits use the `async_trait` crate
- **anyhow::Result**: Unified error handling across the codebase

### Agent Profile Resolution
Profiles resolve in order: project-level (`.rustagent/profiles/{name}.toml`) -> user-level (`~/.config/rustagent/profiles/{name}.toml`) -> built-in. Inheritance via `extends` field with cycle detection.

## Important Notes

### Cargo Edition
`edition = "2024"` requires Rust 1.85.0+.

### Logging
Logs at `~/.local/state/rustagent/logs/` with daily rotation. Use `RUST_LOG=rustagent=debug`.

### Database Location
Default database at `~/.local/share/rustagent/rustagent.db`.

### Version Control
This repo uses both Git and Jujutsu (`.jj/` directory). Be aware of dual VCS.
