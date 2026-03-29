# V2 Architecture Review: Gap Analysis

**Date:** 2026-02-06
**Reviewer:** Claude (technical product owner perspective)
**Document under review:** `docs/plans/v2-architecture.md`

---

## Critical Issues (Will Cause System Failures)

### 1. Hash-Based ID Scheme Is Mathematically Broken

The plan specifies 4 hex characters (16 bits, 65,536 possibilities) truncated from UUID v4 and claims it's "collision-safe enough for <10 concurrent agents." **This is provably wrong.**

| Node Count | P(collision) | Scenario |
|-----------|-------------|----------|
| 50 | 1.85% | Tiny single-agent project |
| 100 | 7.27% | Small project |
| 300 | ~50% | Medium project, 1 agent |
| 600 | ~94% | Medium project, 3 agents |
| 1,500 | ~100% | 10 agents (the stated use case) |

A medium project generates 300-600 nodes (goals + tasks + subtasks + decisions + options + outcomes + observations + revisits). The birthday bound is ~301 IDs for a 50% collision probability. With 10 concurrent agents the situation is dramatically worse, not better, since each agent generates IDs independently.

**Impact:** Collisions hit the PRIMARY KEY constraint. Either INSERTs fail silently (agent loses work), or with upsert semantics, one node overwrites another. The work graph — the central coordination mechanism — becomes corrupt.

**Fix:** Use 8 hex characters (32 bits). At 10,000 nodes the collision probability is 1.16%. This matches the convention git uses for short hashes and is still human-typeable (`ra-a3f8b2c1.1.3`). Alternatively, clarify that the 4-char hash only needs sibling-uniqueness (within one parent) and use the full hierarchical path as the PK.

### 2. SQLite Write Contention Is Not Addressed

The plan says "WAL mode for concurrent reads" but **WAL does not enable concurrent writes**. SQLite remains single-writer even in WAL mode. With 4+ workers writing simultaneously (task claims, status updates, node creation, edge creation), you'll hit `SQLITE_BUSY` errors.

Specific dangers:
- **Transaction upgrade deadlocks**: If a transaction starts as a reader then tries to write, SQLite returns `SQLITE_BUSY` *immediately* without respecting `busy_timeout`
- **"Atomic claim"** (`claim_task`) is described but no implementation strategy is given. A naive read-then-write is NOT atomic without `BEGIN IMMEDIATE`
- **tokio-rusqlite does not serialize writes** — the application must handle this

**Missing from the plan:**
- Write serialization strategy (single connection with mutex? Channel-based write queue?)
- Transaction isolation level choices (`IMMEDIATE` vs `DEFERRED`)
- `busy_timeout` configuration
- Retry logic for `SQLITE_BUSY`
- Checkpoint tuning (`wal_autocheckpoint`)

**Fix:** Add a "Database Concurrency Strategy" section specifying: single `tokio-rusqlite` connection, all write transactions use `BEGIN IMMEDIATE`, `busy_timeout` of 5000ms, and application-level write queue for high-contention operations.

### 3. No Error Handling Strategy for LLM Failures Mid-Task

The plan details worker lifecycle states but never addresses what happens when:
- An LLM API call fails mid-task execution (network error, rate limit, context window exceeded)
- A worker's LLM returns malformed tool calls
- A worker hallucinates and generates invalid graph operations
- A worker exceeds its token budget

The current codebase has `llm/retry.rs` with rate-limit handling, but there's no specification for how the **orchestrator** should handle persistent LLM failures vs transient ones. `max_retries_per_task: 2` is mentioned but retry semantics aren't defined — does it re-spawn a fresh worker? Reuse the same context? Reset the task fully?

---

## Significant Gaps (Incomplete Specifications)

### 4. Agent Profiles: Referenced But Not Specified

The plan mentions "Agent profiles in TOML config" and `builtin_profiles.rs` with planner/coder/reviewer/tester/researcher, but never specifies:
- What fields an `AgentProfile` contains beyond what's in the struct
- What system prompts each built-in profile uses
- How profiles control which tools an agent can access
- How `SecurityScope` maps to profiles
- The TOML schema for custom profiles
- Whether profiles can inherit from/extend built-in profiles

This is one of the most important design surfaces — it determines how agents *behave* — and it's entirely hand-waved.

### 5. Autonomy Levels: Named But Not Defined

`config/autonomy.rs` is listed with types `AutonomyLevel` and `ApprovalGate`. The orchestrator mentions "configurable approval gates." But nowhere does the plan define:
- What autonomy levels exist
- What actions each level permits/restricts
- What approval gates are available
- How gates interact with the orchestrator state machine
- Whether autonomy is per-project, per-goal, or per-agent
- How approval requests are surfaced to users (CLI? Web UI? Both?)

### 6. ContextBuilder: The Glue That's Missing

`context/mod.rs` is supposed to combine AGENTS.md, memories, task details, and decisions into an `AgentContext` for each worker. But there's no specification for:
- How context is prioritized when it exceeds the LLM's context window
- Token budgeting strategy (how much of the window goes to system prompt vs memories vs task details vs conversation history)
- How AGENTS.md sections are matched to a worker's file scope
- Whether context is static (built once at spawn) or dynamic (refreshed during execution)
- How the memory decay thresholds work concretely (age-based? access-based? what are "recent", "older", "ancient"?)

### 7. AGENTS.md Format: Undefined

The plan mentions "AGENTS.md parser + resolver" and "closest-to-file resolution" but never specifies:
- The expected format of AGENTS.md files
- What sections/headings are recognized
- How inheritance works when multiple AGENTS.md files exist in a directory hierarchy
- Whether this follows any existing convention (Cursor rules? Claude's own CLAUDE.md?) or is a new format

### 8. Memory System: Embedding Dimension Mismatch

The schema hardcodes `FLOAT[1536]` for embeddings (OpenAI's `text-embedding-ada-002` dimension). But:
- The plan supports Ollama embeddings, which use different dimensions (e.g., `nomic-embed-text` = 768, `mxbai-embed-large` = 1024)
- There's no strategy for mixed-dimension embeddings
- Switching providers would require re-embedding all existing memories or supporting multiple virtual tables
- No embedding model is specified in the config schema

### 9. Cross-Goal Edge Import: Warning-Not-Error Is Dangerous

The plan says cross-goal edges produce "warnings, not errors" when the referenced goal isn't imported yet. But this means:
- The graph can be in an inconsistent state after import
- Dangling edge references violate referential integrity
- There's no mechanism to later resolve these warnings
- A user could import files in any order and get a silently broken graph

---

## Missing Specifications

### 10. No Testing Strategy

The current codebase has 17 test files. The v2 plan mentions zero tests. For a system this complex, there's no mention of:
- Unit testing approach for graph operations
- Integration testing for multi-agent orchestration
- How to test concurrent worker behavior
- Mock strategies for LLM calls during testing
- How to test the daemon/API endpoints
- Performance/load testing for SQLite under concurrent access
- End-to-end testing strategy

### 11. No Migration Path from V1

The current system uses JSON spec files. The plan says nothing about:
- Whether existing specs can be imported into the new graph
- Whether the CLI remains backward-compatible during transition
- How users transition from `rustagent run <spec>` to `rustagent run --project <name> "goal"`
- Whether the TUI (which is already built with 15 files) is carried forward or abandoned

The TUI is a significant existing investment (ratatui-based, multiple views) that isn't mentioned in the v2 plan at all. Is it replaced by the web UI? Does it coexist?

### 12. No Token/Cost Management

For a system that spawns multiple concurrent LLM workers, there's no mention of:
- Token budget per worker, per task, per goal, or per session
- Cost tracking or reporting
- Circuit breakers if spending exceeds thresholds
- How `max_tokens` interacts with context window management
- Whether different workers can use different models (cheap model for planning, expensive for coding)

### 13. No Observability Beyond WebSocket Events

The plan lists WebSocket events for the web UI but doesn't address:
- Structured logging for the orchestrator and workers
- Metrics collection (task completion rate, agent utilization, retry rates)
- How to debug a misbehaving worker after the fact
- Whether worker LLM conversations are persisted for audit/debugging
- Alerting on failures

### 14. No Git Integration Details

`tools/git.rs` is listed but never specified. For a coding agent, git is critical:
- What git operations are supported? (commit, branch, diff, status, merge?)
- How do parallel workers interact with git? (separate branches? worktrees?)
- What happens when two workers modify the same file through git?
- How does git interact with the file ownership map?
- Is there a strategy for atomic commits per work package?

### 15. No Graceful Degradation

The plan describes the happy path thoroughly but doesn't address:
- What happens if sqlite-vec fails to load? (The plan mentions `instant-distance` as fallback but with zero detail)
- What happens if the embedding provider is unavailable? Does the system work without memory?
- What happens if the daemon crashes while workers are running?
- What happens if disk is full and SQLite can't write?

---

## Questionable Technical Decisions

### 16. sqlite-vec: Pre-v1 With Maintenance Concerns

Research findings:
- Pre-v1 with explicit warning: "expect breaking changes"
- No updates for ~6 months as of recent reports
- **Brute-force only** — no ANN indexing (linear scan)
- Performance degrades significantly beyond 500K vectors
- This is fine for the memory system (small scale), but the "pre-v1 with stale maintenance" risk should be acknowledged

The fallback to `instant-distance` is mentioned once without any detail on how switching would work, what the API differences are, or whether the schema changes.

### 17. Deterministic Orchestrator: Power vs. Adaptability Trade-off

The plan explicitly states the orchestrator is "NOT an LLM agent" but a "deterministic state machine." This is presented as purely beneficial. But it creates blind spots:

- **Work package grouping** requires predicting file scope before execution — but an agent often discovers it needs to modify files not in its original scope
- **Task dependency resolution** is static — but tasks discovered during execution need dynamic re-planning
- A deterministic scheduler can't handle "this task turned out to be three tasks" without a planner agent re-intervening
- The plan's own `NeedsDecision` message implies the orchestrator sometimes needs judgment it can't provide

The plan should specify: what triggers re-planning? How does the orchestrator handle scope changes? Is there a feedback loop where workers can request work package modifications?

### 18. "Hybrid Messaging" Needs More Rigor

The plan says "Orchestrator controls lifecycle + agents can message peers directly" but peer messaging is only mentioned for review workflows. This creates ambiguity:
- Can a coder worker ask another coder worker a question?
- What's the message delivery guarantee? (fire-and-forget? at-least-once?)
- What happens to in-flight messages when a worker is cancelled?
- Is the message bus persisted or in-memory only?

The plan says communication is "primarily through the shared SQLite database" but then defines a `WorkerMessage` enum with multiple variants for direct messaging. Which is it?

### 19. Session Model: Handoff Notes Are LLM-Generated

"Orchestrator generates handoff notes summarizing: what was done, what's left, blockers, decisions made." But the orchestrator is explicitly NOT an LLM agent. So who generates these notes? If it's a final LLM call, that's not specified. If it's template-based from graph state, that's not specified either.

---

## Structural Concerns

### 20. Phase 1 Is Too Large

Phase 1 contains 11 items including: full database schema, project management, complete graph model with 7 node types and 7 edge types, dependency resolution, ready surfacing, ADR export, TOML import/export/diff, session management, agent trait definition, runtime refactoring, config extension, cherry-picking existing modules, all graph tools, CLI wiring.

This is realistically 3-4 phases of work collapsed into one. There's no way to get feedback on the database design before building the TOML interchange format on top of it. Recommended split:
- Phase 1a: Database + schema + basic CRUD
- Phase 1b: Graph model + node lifecycle + dependency resolution
- Phase 1c: Session model + ADR export + TOML interchange
- Phase 1d: Agent trait + runtime refactor + CLI

### 21. Web UI Tech Choices Need Justification

"React 19 + TypeScript + Bun + Vite" is stated without discussing:
- Why React over lighter alternatives (the UI is essentially a dashboard)
- State management approach (the stores are listed but no library is specified — Zustand? Redux? React Context?)
- Whether server-side rendering matters
- Bundle size considerations
- Whether Bun is production-ready enough for the build toolchain

### 22. No Versioning/Schema Migration Strategy

The database schema is defined once. There's no mention of:
- Schema versioning
- Migration tooling (hand-rolled? refinery? sqlx-migrate?)
- Backward compatibility when schema changes
- How TOML export format versioning works (`version = 1` is mentioned but no evolution strategy)

---

## Summary

| Category | Count | Severity |
|----------|-------|----------|
| Critical (will cause failures) | 3 | Must fix before implementation |
| Significant gaps | 6 | Will block implementation of specific phases |
| Missing specifications | 6 | Will require design decisions during implementation |
| Questionable decisions | 4 | Should be revisited with explicit trade-off analysis |
| Structural concerns | 3 | Affect project execution, not correctness |

**Top 5 action items:**
1. Fix the ID scheme (8 hex chars minimum, or sibling-only uniqueness with full-path PKs)
2. Add a database concurrency section (write serialization, `BEGIN IMMEDIATE`, retry logic)
3. Specify agent profiles and autonomy levels fully
4. Split Phase 1 into 4 sub-phases
5. Add LLM failure handling and token budget management to the orchestrator spec

---

## Research Sources

### sqlite-vec
- [GitHub - asg017/sqlite-vec](https://github.com/asg017/sqlite-vec)
- [Introducing sqlite-vec v0.1.0](https://alexgarcia.xyz/blog/2024/sqlite-vec-stable-release/index.html)
- [Using sqlite-vec in Rust](https://alexgarcia.xyz/sqlite-vec/rust.html)
- [API Reference](https://alexgarcia.xyz/sqlite-vec/api-reference.html)
- [GitHub - djc/instant-distance](https://github.com/djc/instant-distance)

### SQLite Concurrency
- [SQLite WAL Documentation](https://sqlite.org/wal.html)
- [SQLite File Locking (Locking v3)](https://sqlite.org/lockingv3.html)
- [Bert Hubert - SQLITE_BUSY Despite Timeout](https://berthub.eu/articles/posts/a-brief-post-on-sqlite3-database-locked-despite-timeout/)
- [tenthousandmeters - SQLite Concurrent Writes](https://tenthousandmeters.com/blog/sqlite-concurrent-writes-and-database-is-locked-errors/)
- [tokio-rusqlite Documentation](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/)
- [rusqlite Transaction Behavior](https://docs.rs/rusqlite/latest/rusqlite/enum.TransactionBehavior.html)
- [SQLite Atomic Commit](https://sqlite.org/atomiccommit.html)

### Birthday Problem / ID Collisions
- Standard birthday problem formula: P(collision) = 1 - e^(-N(N-1) / (2D)) where D = 2^16 = 65,536
- Birthday bound for 50% collision: N = sqrt(2D * ln(2)) ≈ 301 IDs
