# Rustagent Design Document

A Rust implementation of the Ralph Loop pattern for autonomous AI agent development.

## Overview

Rustagent is a CLI tool with two modes:
1. **Planning Agent** - Interactive brainstorming to create structured specs
2. **Ralph Loop** - Autonomous task execution with fresh context per iteration

## CLI Interface

```
rustagent init [--spec-dir <path>]     # Initialize spec directory
rustagent plan [--spec-dir <path>]     # Interactive planning mode
rustagent run <spec-file> [--max-iterations <n>]  # Ralph loop execution
```

## Configuration

Location: `rustagent.toml` (project root) or `~/.config/rustagent/config.toml`

```toml
[llm]
provider = "anthropic"  # or "openai", "ollama"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${ANTHROPIC_API_KEY}"

[openai]
api_key = "${OPENAI_API_KEY}"

[ollama]
base_url = "http://localhost:11434"

[rustagent]
spec_dir = "specs"        # Default spec directory
max_iterations = null     # null = unlimited
```

CLI flags override config values.

## LLM Client Architecture

Trait-based abstraction for multiple providers:

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<Message>, tools: &[ToolDefinition])
        -> Result<Response>;
}
```

Implementations: `AnthropicClient`, `OpenAiClient`, `OllamaClient`

## Tool System

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult>;
}
```

### Available Tools

| Tool | Used By | Description |
|------|---------|-------------|
| `read_file` | Both | Read file contents |
| `write_file` | Both | Write/create a file |
| `edit_file` | Both | Targeted find/replace edits |
| `list_files` | Both | List directory contents |
| `run_command` | Both | Execute shell command |
| `web_search` | Planning only | Search the web |

## Spec Format (JSON)

```json
{
  "name": "feature-name",
  "description": "High-level description",
  "branch_name": "feature/feature-name",
  "created_at": "2026-01-19T12:00:00Z",
  "tasks": [
    {
      "id": "task-1",
      "title": "Implement X",
      "description": "Detailed description",
      "acceptance_criteria": ["Criterion 1", "Criterion 2"],
      "status": "pending",
      "blocked_reason": null,
      "completed_at": null
    }
  ],
  "learnings": [
    "Discovered that the API requires auth header"
  ]
}
```

Task status values: `pending`, `in_progress`, `complete`, `blocked`

## Planning Agent Flow

1. Start interactive conversation
2. Ask clarifying questions one at a time
3. Use web_search for research as needed
4. Break work into discrete tasks
5. Write spec JSON to spec directory
6. Show summary and handoff instructions

## Ralph Loop Algorithm

```
1. Load spec file
2. Find first task with status = "pending"
3. If no pending tasks → exit success
4. Mark task as "in_progress", save spec
5. Build fresh context:
   - System prompt
   - Full spec for context
   - Specific task to work on
   - Recent learnings
6. Run agent on the task
7. If agent signals stuck:
   - Mark task status as "blocked"
   - Record blocked_reason
   - Append to learnings
   - Exit with error
8. If agent completes task:
   - Mark task as "complete"
   - Append learnings
9. Save updated spec
10. Wait delay (default 2 seconds)
11. If max_iterations reached → exit with warning
12. Go to step 2
```

Key properties:
- Fresh LLM context each iteration
- Persistence via spec file and git history
- One task per iteration
- Immediate exit on blocked state

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── config.rs            # Configuration loading
├── llm/
│   ├── mod.rs           # LlmClient trait
│   ├── anthropic.rs     # Anthropic implementation
│   ├── openai.rs        # OpenAI implementation
│   └── ollama.rs        # Ollama implementation
├── tools/
│   ├── mod.rs           # Tool trait and registry
│   ├── file.rs          # File operations
│   ├── shell.rs         # Shell command execution
│   └── web.rs           # Web search
├── spec.rs              # Spec file handling
├── planning/
│   └── mod.rs           # Planning agent
└── ralph/
    └── mod.rs           # Ralph loop
```

## Dependencies

- `clap` - CLI parsing
- `tokio` - async runtime
- `reqwest` - HTTP client
- `serde` / `serde_json` - serialization
- `anyhow` - error handling
- `toml` - config parsing

## References

- [Everything is a Ralph Loop](https://ghuntley.com/loop/)
- [Ralph Wiggum as a Software Engineer](https://ghuntley.com/ralph/)
- [snarktank/ralph](https://github.com/snarktank/ralph)
