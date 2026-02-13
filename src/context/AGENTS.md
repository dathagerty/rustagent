# Context Module

Last verified: 2026-02-13

## Purpose
Assembles structured system prompts for agents from their AgentContext. Handles token budget management to prioritize critical information (role, tasks, rules) over optional context (sessions, decisions, conventions) when prompts are too large.

## Contracts
- **Exposes**: `ContextBuilder` (plain + budget-aware), `ContextBudget` (default: 4000 tokens), `ReadAgentsMdTool`, `resolve_agents_md()`
- **Guarantees**: Required sections (Role, Task, Dependencies, Previous Attempt, Rules) are never trimmed. Optional sections are dropped in priority order when over budget: Session Continuity > Active Decisions > Observations > Project Conventions (lowest). Token estimate uses 1 token per 4 chars. `ReadAgentsMdTool` only reads AGENTS.md files (rejects other filenames). `resolve_agents_md` returns relative paths with heading line counts.
- **Expects**: A populated `AgentContext`. `ReadAgentsMdTool` needs a valid project path or AGENTS.md file path.

## Dependencies
- **Uses**: `agent::AgentContext`, `graph::GraphNode`, `tools::Tool` trait
- **Used by**: `agent::runtime` (builds system prompt each run), `tools::factory` (registers ReadAgentsMdTool)
- **Boundary**: Does NOT access the database or LLM directly

## Key Decisions
- Budget-aware assembly: Required sections always included; optional sections trimmed by priority when over budget
- Token estimation at 4 chars/token: Simple heuristic avoids tokenizer dependency
- ReadAgentsMdTool accepts directory paths: Agents can pass `src/auth` instead of `src/auth/AGENTS.md`

## Invariants
- `build_system_prompt_with_budget` always includes Role, Task, Dependencies, Previous Attempt, and Rules sections
- `resolve_agents_md` walks from file's directory up to project root, deduplicating paths
- AGENTS.md heading summaries include line counts: `"Section Name (N lines)"`

## Key Files
- `mod.rs` - ContextBuilder, ContextBudget, ReadAgentsMdTool
- `agents_md.rs` - resolve_agents_md(), extract_heading_summaries()
