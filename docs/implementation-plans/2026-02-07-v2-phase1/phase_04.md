# Rustagent V2 Phase 1d: Agent Runtime + Single-Agent Execution

**Goal:** Cherry-pick existing modules (LLM, security, tools), define the Agent trait and AgentProfile system, refactor the Ralph loop into a generic AgentRuntime with error handling (confusion counter, token budget), implement profile resolution, and wire up `rustagent run` for single-agent execution.

**Architecture:** The AgentRuntime is a generic agentic loop (LLM call -> tool execution -> repeat) that replaces the v1 Ralph loop. It's parameterized by an AgentProfile (which controls system prompt, allowed tools, security scope, LLM config) and an AgentContext (task details, decisions, handoff notes). Error handling adds a confusion counter for bad tool calls, configurable consecutive failure thresholds, and per-worker token budget tracking.

**Tech Stack:** Rust (edition 2024), async-trait, tokio, serde/serde_json, chrono, uuid, anyhow, walkdir, glob

**Scope:** Phase 4 of 4 from the v2 architecture (Phase 1d: Agent Runtime + Single-Agent Execution)

**Codebase verified:** 2026-02-07

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

**Depends on:** Phase 1a (database), Phase 1b (graph store, graph tools), Phase 1c (sessions, context decay)

**Deferred to later phases:**
- `src/config/autonomy.rs` (AutonomyLevel, ApprovalGate types) — architecture Phase 5. The `run` command in this phase does not support `--autonomy` flag. Autonomy enforcement requires the orchestrator (Phase 2) and approval gate system (Phase 5).
- `src/tools/search.rs` (code search tool) — architecture Phase 5. The `search` CLI command in Phase 1b covers FTS5 graph node search only. File content search for agents is deferred.
- `pulldown-cmark` (AGENTS.md parsing) — simple string-based heading extraction is sufficient for Phase 1d. Full markdown parsing deferred if needed.
- `tokio-util` (CancellationToken) — `Agent::cancel()` is a no-op stub in Phase 1d (single-agent mode). CancellationToken integration deferred to Phase 2 (multi-agent orchestration).

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P1d.AC1: Cherry-pick and adapt existing modules
- **P1d.AC1.1 Success:** LLM module (`src/llm/`) compiles in new structure with no TUI dependencies
- **P1d.AC1.2 Success:** Security module (`src/security/`) compiles with new `SecurityScope` type added
- **P1d.AC1.3 Success:** Tools module (`src/tools/`) compiles with graph_tools integrated into the registry

### P1d.AC2: Agent trait and types
- **P1d.AC2.1 Success:** `Agent` trait defined with `id()`, `profile()`, `run(ctx) -> AgentOutcome`, `cancel()`
- **P1d.AC2.2 Success:** `AgentContext` struct contains task details, relevant decisions, handoff notes, AGENTS.md summaries, profile
- **P1d.AC2.3 Success:** `AgentOutcome` enum covers Completed, Blocked, Failed, TokenBudgetExhausted

### P1d.AC3: Agent profiles
- **P1d.AC3.1 Success:** `AgentProfile` struct with name, extends, role, system_prompt, allowed_tools, security, llm config, turn_limit, token_budget
- **P1d.AC3.2 Success:** 5 built-in profiles defined: planner, coder, reviewer, tester, researcher
- **P1d.AC3.3 Success:** Custom profiles loaded from `.rustagent/profiles/*.toml` (project-level) and `~/.config/rustagent/profiles/*.toml` (user-level)
- **P1d.AC3.4 Success:** Profile resolution: project-level > user-level > built-in. First match wins.
- **P1d.AC3.5 Success:** Inheritance via `extends`: scalar fields replaced, list fields replaced, system_prompt appended

### P1d.AC4: AgentRuntime (agentic loop)
- **P1d.AC4.1 Success:** AgentRuntime runs the LLM call -> tool execution -> repeat loop
- **P1d.AC4.2 Success:** Confusion counter: after N consecutive bad tool calls (configurable), worker signals blocked
- **P1d.AC4.3 Success:** Token budget: at warning threshold (default 80%), injects "wrap up" system message. At 100%, force-stops with partial completion.
- **P1d.AC4.4 Success:** Consecutive LLM failure threshold: after N failures (configurable), worker signals blocked
- **P1d.AC4.5 Success:** Turn limit: worker stops after max turns with partial completion report

### P1d.AC5: Context assembly
- **P1d.AC5.1 Success:** ContextBuilder assembles compact structured context from task details, decisions, handoff notes, observations, AGENTS.md
- **P1d.AC5.2 Success:** AGENTS.md files resolved by closest-to-file rule per agents.md spec

### P1d.AC6: Single-agent CLI
- **P1d.AC6.1 Success:** `rustagent run --project <name> "<goal>"` creates a goal node, creates a session, runs a single coder agent, and records the outcome
- **P1d.AC6.2 Success:** Profile selection via `--profile <name>` flag (defaults to coder)

---

<!-- START_TASK_1 -->
### Task 1: Clean up cherry-picked modules for v2 compatibility

**Verifies:** P1d.AC1.1, P1d.AC1.2, P1d.AC1.3

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/ralph/mod.rs` — remove `use crate::tui::messages::{AgentMessage, AgentSender};` and the `run_with_sender` and `execute_task_with_sender` methods (TUI removed in Phase 1a Task 2)
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/tools/factory.rs` — add `SqliteGraphStore` parameter, register graph tools alongside existing tools
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/security/mod.rs` — no changes needed yet (SecurityScope added in Task 3)

**Implementation:**

In `ralph/mod.rs`:
- Remove the import of `crate::tui::messages`
- Remove the `run_with_sender` method entirely
- Remove the `execute_task_with_sender` method entirely
- The remaining `run` and `execute_task` methods stay as-is — they'll be replaced by AgentRuntime in Task 5, but the v1 `Run` command should still work in the meantime.

In `tools/factory.rs`:
- Add a new function `create_v2_registry(validator, permission_handler, graph_store)` that creates the default registry AND registers all graph tools from `graph_tools.rs`. Keep the existing `create_default_registry` for backward compatibility with v1 commands.
- The graph tools need `Arc<SqliteGraphStore>` passed in, and the agent's ID for tools that need it (like `claim_task`).

**Verification:**

Run: `cargo check`
Expected: Compiles cleanly

Run: `cargo test`
Expected: All existing tests still pass

**Commit:** `refactor: clean up modules for v2 compatibility, add v2 tool registry factory`

<!-- END_TASK_1 -->

<!-- START_SUBCOMPONENT_A (tasks 2-3) -->

<!-- START_TASK_2 -->
### Task 2: Agent trait and AgentOutcome

**Verifies:** P1d.AC2.1, P1d.AC2.3

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/mod.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod agent;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/agent_types_test.rs`

**Implementation:**

`src/agent/mod.rs`:

```rust
pub type AgentId = String;

#[async_trait]
pub trait Agent: Send + Sync {
    fn id(&self) -> &AgentId;
    fn profile(&self) -> &AgentProfile;
    async fn run(&self, ctx: AgentContext) -> Result<AgentOutcome>;
    fn cancel(&self);  // No-op stub in Phase 1d (single-agent). CancellationToken integration deferred to Phase 2.
}

pub enum AgentOutcome {
    Completed { summary: String },
    Blocked { reason: String },
    Failed { error: String },
    TokenBudgetExhausted { summary: String, tokens_used: usize },
}

pub struct AgentContext {
    pub work_package_tasks: Vec<GraphNode>,
    pub relevant_decisions: Vec<GraphNode>,
    pub handoff_notes: Option<String>,
    pub agents_md_summaries: Vec<(String, String)>,  // (path, heading summary)
    pub profile: AgentProfile,
    pub project_path: PathBuf,
    pub graph_store: Arc<dyn GraphStore>,
}
```

Declare sub-modules: `pub mod profile;`, `pub mod runtime;`, `pub mod builtin_profiles;`

**Testing:**

- P1d.AC2.1: Verify Agent trait compiles (it's a trait, so just verify a mock can implement it)
- P1d.AC2.3: Verify AgentOutcome variants can be constructed and matched

**Verification:**
Run: `cargo test agent_types_test`
Expected: All tests pass

**Commit:** `feat(agent): Agent trait, AgentId, AgentContext, AgentOutcome types`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: AgentProfile type and SecurityScope

**Verifies:** P1d.AC3.1, P1d.AC1.2

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/profile.rs`
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/security/scope.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/security/mod.rs` — add `pub mod scope;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/profile_test.rs`

**Implementation:**

`src/security/scope.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScope {
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_commands: Vec<String>,
    pub read_only: bool,
    pub can_create_files: bool,
    pub network_access: bool,
}
```

Implement `Default` with permissive defaults (all paths allowed, not read-only, etc.) so built-in profiles can override specific fields.

`src/agent/profile.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    pub extends: Option<String>,
    pub role: String,
    pub system_prompt: String,
    pub allowed_tools: Vec<String>,
    pub security: SecurityScope,
    #[serde(default)]
    pub llm: ProfileLlmConfig,
    pub turn_limit: Option<usize>,
    pub token_budget: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileLlmConfig {
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<usize>,
}
```

Implement `AgentProfile::apply_inheritance(&mut self, parent: &AgentProfile)`:
1. For scalar fields: only override if `self` has a meaningful value (non-empty string, Some, etc.)
2. For list fields (allowed_tools, allowed_paths, etc.): child replaces parent entirely (not merged)
3. For system_prompt: append child to parent with `\n\n## Project-Specific Instructions\n` separator
4. For optional fields (turn_limit, token_budget, llm): child Some wins, falls through to parent if None

**Testing:**

Tests must verify:
- P1d.AC3.1: AgentProfile deserializes from TOML string matching the format in the architecture doc
- P1d.AC1.2: SecurityScope deserializes correctly; Default gives permissive scope
- Inheritance: parent with `role = "coder"`, child with `role = "rust-coder"` → child role wins. Parent with `allowed_tools = ["file", "shell"]`, child with `allowed_tools = ["file"]` → child list wins (not merged). Parent system_prompt + child system_prompt → concatenated with separator.

**Verification:**
Run: `cargo test profile_test`
Expected: All tests pass

**Commit:** `feat(agent): AgentProfile type with SecurityScope and inheritance`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->

<!-- START_TASK_4 -->
### Task 4: Add token tracking fields to Response type

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/llm/mod.rs` — add `input_tokens` and `output_tokens` to `Response`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/llm/anthropic.rs` — populate token fields from API response
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/llm/openai.rs` — populate token fields from API response
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/llm/ollama.rs` — populate token fields (None if not available)
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/llm/mock.rs` — return configurable token counts

**Implementation:**

Add to `Response` in `src/llm/mod.rs`:

```rust
pub struct Response {
    pub content: ResponseContent,
    pub stop_reason: Option<String>,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
}
```

Update each provider to extract token usage from their API responses:
- Anthropic: `response.usage.input_tokens` and `response.usage.output_tokens`
- OpenAI: `response.usage.prompt_tokens` and `response.usage.completion_tokens`
- Ollama: `response.eval_count` for output, `response.prompt_eval_count` for input (if available)
- Mock: Add `pub fn set_token_counts(&self, input: usize, output: usize)` to configure returned values

**Verification:**

Run: `cargo test`
Expected: All existing tests pass (Response construction sites need updating with the new fields)

**Commit:** `feat(llm): add token usage tracking to Response type`

<!-- END_TASK_4 -->

<!-- START_SUBCOMPONENT_B (tasks 5-6) -->

<!-- START_TASK_5 -->
### Task 5: Built-in profiles and profile resolution

**Verifies:** P1d.AC3.2, P1d.AC3.3, P1d.AC3.4, P1d.AC3.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/builtin_profiles.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/profile.rs` — add `resolve_profile` function
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/profile_test.rs` (extend)

**Implementation:**

`src/agent/builtin_profiles.rs`:

Define 5 functions, each returning an `AgentProfile`:
- `pub fn planner() -> AgentProfile` — read-only, graph+signal tools, system prompt for task breakdown
- `pub fn coder() -> AgentProfile` — write access (scoped), file+shell+graph+signal tools, system prompt for implementation
- `pub fn reviewer() -> AgentProfile` — read-only, file+shell+graph+signal tools, system prompt for code review
- `pub fn tester() -> AgentProfile` — write access (test dirs), file+shell+graph+signal tools, system prompt for test writing
- `pub fn researcher() -> AgentProfile` — read-only, file+shell+search+graph+signal tools, system prompt for information gathering

Each profile's system_prompt follows the template from the architecture (lines 1507-1525).

In `profile.rs`, add:

```rust
pub fn resolve_profile(
    name: &str,
    project_path: Option<&Path>,
) -> Result<AgentProfile> {
    // 1. Project-level: .rustagent/profiles/{name}.toml
    if let Some(path) = project_path {
        let profile_path = path.join(".rustagent/profiles").join(format!("{}.toml", name));
        if profile_path.exists() {
            let content = std::fs::read_to_string(&profile_path)?;
            let mut profile: AgentProfile = toml::from_str(&content)?;
            if let Some(parent_name) = &profile.extends.clone() {
                let parent = resolve_profile(parent_name, project_path)?;
                profile.apply_inheritance(&parent);
            }
            return Ok(profile);
        }
    }

    // 2. User-level: ~/.config/rustagent/profiles/{name}.toml
    if let Some(config_dir) = dirs::config_dir() {
        let profile_path = config_dir.join("rustagent/profiles").join(format!("{}.toml", name));
        if profile_path.exists() {
            let content = std::fs::read_to_string(&profile_path)?;
            let mut profile: AgentProfile = toml::from_str(&content)?;
            if let Some(parent_name) = &profile.extends.clone() {
                let parent = resolve_profile(parent_name, project_path)?;
                profile.apply_inheritance(&parent);
            }
            return Ok(profile);
        }
    }

    // 3. Built-in
    match name {
        "planner" => Ok(builtin_profiles::planner()),
        "coder" => Ok(builtin_profiles::coder()),
        "reviewer" => Ok(builtin_profiles::reviewer()),
        "tester" => Ok(builtin_profiles::tester()),
        "researcher" => Ok(builtin_profiles::researcher()),
        _ => anyhow::bail!("Unknown profile: {}", name),
    }
}
```

Add cycle detection: track resolved names in a `HashSet` and error if a name appears twice.

**Testing:**

Tests must verify:
- P1d.AC3.2: `resolve_profile("coder", None)` returns the built-in coder profile
- P1d.AC3.3: Create a tempdir with `.rustagent/profiles/custom.toml`. Resolve "custom" with that project path. Returns the custom profile.
- P1d.AC3.4: Create a project-level profile named "coder" that overrides the built-in. Resolve "coder" with that project path. Project-level wins.
- P1d.AC3.5: Create a custom profile with `extends = "coder"`. Resolve it. Verify inheritance applied correctly (system_prompt appended, list fields replaced, scalar fields overridden).

**Verification:**
Run: `cargo test profile_test`
Expected: All tests pass

**Commit:** `feat(agent): 5 built-in profiles and profile resolution chain`

<!-- END_TASK_5 -->

<!-- START_TASK_6 -->
### Task 6: AgentRuntime — agentic loop with error handling

**Verifies:** P1d.AC4.1, P1d.AC4.2, P1d.AC4.3, P1d.AC4.4, P1d.AC4.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/agent/runtime.rs`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/agent_runtime_test.rs`

**Implementation:**

`src/agent/runtime.rs`:

```rust
pub struct AgentRuntime {
    client: Arc<dyn LlmClient>,
    tools: ToolRegistry,
    profile: AgentProfile,
    config: RuntimeConfig,
}

pub struct RuntimeConfig {
    pub max_turns: usize,                       // Default: 100
    pub max_consecutive_llm_failures: usize,    // Default: 3
    pub max_consecutive_tool_failures: usize,   // Default: 3
    pub token_budget: usize,                    // Default: 200_000
    pub token_budget_warning_pct: u8,           // Default: 80
}
```

`impl AgentRuntime`:
- `pub fn new(client, tools, profile, config) -> Self`
- `pub async fn run(&self, context: AgentContext) -> Result<AgentOutcome>`:

The loop is a refactored version of `RalphLoop::execute_task`:

1. Build system message from context (using ContextBuilder — Task 6)
2. Loop for up to `max_turns`:
   a. Call `client.chat(messages, tools)`
   b. On LLM error: increment `consecutive_llm_failures`. If >= threshold, return `AgentOutcome::Blocked`.
   c. On success: reset `consecutive_llm_failures` to 0.
   d. Track token usage: `cumulative_tokens += response.input_tokens.unwrap_or(0) + response.output_tokens.unwrap_or(0)` (token fields added to `Response` in Task 4)
   e. If cumulative_tokens >= warning threshold and not yet warned: inject system message "You are approaching your token budget. Wrap up your current work and signal completion."
   f. If cumulative_tokens >= budget: return `AgentOutcome::TokenBudgetExhausted`
   g. Handle tool calls: execute each via registry. On unknown tool or parse error, increment `consecutive_tool_failures` and send error back to LLM. On success, reset counter.
   h. If `consecutive_tool_failures` >= threshold: return `AgentOutcome::Blocked`
   i. Check for signal_completion tool call — return appropriate `AgentOutcome`
3. If loop exhausts max_turns: return `AgentOutcome::Completed` with summary "turn limit reached"

Key difference from Ralph loop: error responses go back to the LLM as tool results (not panics), giving it a chance to self-correct.

**Testing:**

Tests use `MockLlmClient` from `src/llm/mock.rs`:

- P1d.AC4.1: Queue a text response then a signal_completion tool call. Run. Returns `AgentOutcome::Completed`.
- P1d.AC4.2: Queue 3 consecutive responses with invalid tool calls (unknown tool name). Run. Returns `AgentOutcome::Blocked` with reason mentioning tool failures.
- P1d.AC4.3: Mock responses that consume tokens. Set budget to 1000 with warning at 80%. Verify "wrap up" message injected at 800 tokens. Set budget to 500. Verify `TokenBudgetExhausted` returned.
- P1d.AC4.4: Queue 3 consecutive errors from the mock client. Run. Returns `AgentOutcome::Blocked` with LLM failure reason.
- P1d.AC4.5: Set max_turns to 3. Queue responses that never signal completion. Run. Returns after 3 turns.

**Verification:**
Run: `cargo test agent_runtime_test`
Expected: All tests pass

**Commit:** `feat(agent): AgentRuntime with confusion counter, token budget, and failure thresholds`

<!-- END_TASK_6 -->
<!-- END_SUBCOMPONENT_B -->

<!-- START_SUBCOMPONENT_C (tasks 7-8) -->

<!-- START_TASK_7 -->
### Task 7: ContextBuilder and AGENTS.md resolution

**Verifies:** P1d.AC5.1, P1d.AC5.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/Cargo.toml` — add `walkdir` and `glob` dependencies
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/context/mod.rs`
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/context/agents_md.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod context;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/context_test.rs`

**Prerequisite:** Add `walkdir = "2"` and `glob = "0.3"` to `[dependencies]` in Cargo.toml. Run `cargo check` to verify.

**Implementation:**

`src/context/agents_md.rs`:

```rust
pub struct AgentsMdSummary {
    pub path: PathBuf,
    pub headings: Vec<String>,  // Top-level headings extracted
}
```

`pub fn resolve_agents_md(project_root: &Path, file_scope: &[PathBuf]) -> Result<Vec<AgentsMdSummary>>`:
1. Start at project root
2. For each file in scope, walk directory hierarchy from root toward the file
3. At each directory level, check for `AGENTS.md` (case-sensitive)
4. Extract top-level headings (lines starting with `# `) using simple string parsing — no full markdown parser needed for heading extraction
5. Return collected summaries, deduplicated, with closest-to-file ordering

`src/context/mod.rs`:

```rust
pub struct ContextBuilder;

impl ContextBuilder {
    pub fn build_system_prompt(ctx: &AgentContext) -> String
}
```

Also create a `ReadAgentsMdTool` implementing the existing `Tool` trait, so agents can call `read_agents_md(path)` during their agentic loop to get the full AGENTS.md content on demand (the system prompt only includes heading summaries). The tool takes a `path` parameter, reads the AGENTS.md file at that path, and returns its full contents. Register this tool in the v2 tool registry (`tools/factory.rs`).

`build_system_prompt` assembles the compact structured format from the architecture (lines 1626-1654):

```
## Role
{profile.role}

## Task
[TASK] {task.id} | {task.title} | priority={task.priority}
[CRITERIA] {acceptance_criteria, semicolon-separated}
...

## Session Continuity
[HANDOFF] {handoff_notes}

## Active Decisions
[DECISION] {id} | {title} | chosen: ...
...

## Relevant Observations (use query_nodes(id) for full detail)
- {node_id}: {one-line summary}
...

## Project Conventions (use read_agents_md(path) for full text)
- {path}: {heading1}, {heading2} ...

## Rules
{profile.system_prompt rules section}
```

**Testing:**

Tests must verify:
- P1d.AC5.1: Given an AgentContext with task, decisions, handoff notes — output contains all sections with correct formatting
- P1d.AC5.2: Create a tempdir with `AGENTS.md` at root and `src/AGENTS.md`. Resolve for scope `["src/auth/handler.rs"]`. Returns both files with `src/AGENTS.md` closer.

**Verification:**
Run: `cargo test context_test`
Expected: All tests pass

**Commit:** `feat(context): ContextBuilder with compact structured format and AGENTS.md resolution`

<!-- END_TASK_7 -->

<!-- START_TASK_8 -->
### Task 8: Wire up `rustagent run` for single-agent execution

**Verifies:** P1d.AC6.1, P1d.AC6.2

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — replace v1 `Run` command with v2 version

**Implementation:**

Replace the existing `Run` command:

```rust
/// Execute a goal with an agent
Run {
    /// Goal description
    goal: String,
    /// Agent profile to use
    #[arg(long, default_value = "coder")]
    profile: String,
    /// Maximum iterations
    #[arg(long)]
    max_iterations: Option<usize>,
},
```

In the match arm for `Commands::Run`:
1. Resolve project (from `--project` flag or cwd)
2. Open database
3. Create goal node in the graph (using `SqliteGraphStore::create_node`)
4. Create a session (using `SessionStore::create_session`)
5. Resolve profile (using `resolve_profile`)
6. Build `AgentContext` from the goal, profile, and session
7. Create `AgentRuntime` with the profile's LLM config
8. Run the runtime
9. Handle `AgentOutcome` — update task nodes, end session, print result

This is the first end-to-end integration: CLI -> database -> graph -> profile -> runtime -> LLM -> tools -> graph updates -> session end.

For this phase (single-agent), there's no orchestrator — the CLI directly creates one agent and runs it. The orchestrator comes in Phase 2 (multi-agent).

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Manual verification (requires LLM API key):
```
cargo run -- project add test-proj .
cargo run -- run --project test-proj "Create a hello world program"
```
Expected: Agent creates tasks, attempts to execute them, records outcome.

**Commit:** `feat(cli): v2 run command with single-agent execution`

<!-- END_TASK_8 -->
<!-- END_SUBCOMPONENT_C -->
