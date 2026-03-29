# Agent Module

Last verified: 2026-02-13

## Purpose
Defines the agent abstraction and runtime loop for autonomous task execution. Agents are configured via profiles that control their role, allowed tools, security scope, LLM settings, and resource budgets.

## Contracts
- **Exposes**: `Agent` trait, `AgentProfile`, `AgentContext`, `AgentOutcome`, `AgentRuntime`, `resolve_profile()`, 5 built-in profiles, `Orchestrator` (multi-agent coordinator)
- **Guarantees**: Runtime stops on token budget exhaustion (returns `TokenBudgetExhausted`). Consecutive LLM/tool failures trigger `Blocked` outcome (configurable thresholds). Profile inheritance detects cycles. `signal_completion` tool ends the loop cleanly. Orchestrator retries failed tasks with previous error context injected. Failed tasks cascade-block dependents; successful retries auto-unblock them.
- **Expects**: An `Arc<dyn LlmClient>`, a `ToolRegistry`, and an `AgentContext` with work package tasks, graph store access, and optional `previous_attempt`/`dependency_statuses` fields.

## Dependencies
- **Uses**: `llm::LlmClient`, `tools::ToolRegistry`, `context::ContextBuilder` (budget-aware), `graph::GraphNode`, `graph::store::GraphStore`, `security::SecurityScope`
- **Used by**: `main.rs` (V2 `run` command wires up the runtime), orchestrator coordinates multi-agent execution
- **Boundary**: Does NOT directly access the database; uses `GraphStore` trait

## Key Decisions
- Profile resolution chain (project -> user -> built-in): Enables per-project customization without forking built-ins
- Inheritance via `extends`: system_prompt appends (child after parent), lists replace, optionals fall through
- SecurityScope per profile: Each agent type has explicit path/command/network restrictions
- Confusion counter pattern: Consecutive failures tracked separately for LLM and tool errors
- Error recovery: On task failure, orchestrator stores error in `metadata["previous_attempt"]` and increments `retry_count`; on retry the error context is injected into the agent's system prompt

## Invariants
- AgentOutcome is always returned (never panics): Completed, Blocked, Failed, or TokenBudgetExhausted
- Token budget warning fires at 80% (configurable), hard stop at 100%
- Built-in profiles: planner (read-only, graph-only), coder (file+shell+graph), reviewer (read-only), tester (file+shell+graph), researcher (read-only)
- Failed tasks cascade-block dependents via `metadata["blocker_task_id"]`; unblock checks use this key (not string parsing)

## Key Files
- `mod.rs` - Agent trait, AgentId, AgentContext (incl. previous_attempt, dependency_statuses), AgentOutcome enum
- `profile.rs` - AgentProfile struct, ProfileLlmConfig, resolve_profile() with cycle detection
- `builtin_profiles.rs` - planner(), coder(), reviewer(), tester(), researcher()
- `runtime.rs` - AgentRuntime, RuntimeConfig (defaults: 100 turns, 200k tokens, 3 failure threshold)
- `orchestrator.rs` - Orchestrator: scheduling, work packages, error recovery (retry/cascade/unblock)
