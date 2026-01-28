# Rustagent

**It's a pun on rustacean.**

Rustagent is an AI agent framework for autonomous task execution in Rust. It uses a two-phase approach: a Planning Agent that breaks down high-level goals into executable tasks, and the Ralph Loop that executes those tasks iteratively with tool access.

The canonical [repository](https://tangled.org/dathagerty.com/rustagent) for this project is on [tangled.org](https://tangled.org).
It is [mirrored](https://github.com/dathagerty/rustagent) on [GitHub](https://github.com) for discoverability.

## Features

- **Interactive TUI**: Terminal user interface built with ratatui for managing specs and monitoring agent execution
- **Planning Agent**: Converts high-level specifications into structured execution plans
- **Ralph Loop**: Autonomous execution agent with Read-Act-Learn-Plan-Help cycle
- **Multiple LLM Support**: Works with Anthropic (Claude), OpenAI (GPT), and Ollama (local models)
- **Tool System**: Extensible tools for file operations and shell commands with security validation
- **Permission System**: CLI prompts for sensitive operations with path and command validation
- **Structured Specs**: JSON-based specification format with tasks, acceptance criteria, and learnings

## Installation

### Prerequisites

- Rust 1.85.0 or later (for Rust edition 2024 support)
- An API key for your chosen LLM provider (or Ollama running locally)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/rustagent.git
cd rustagent

# Build the project
cargo build --release

# The binary will be available at target/release/rustagent
```

### Install Locally

```bash
cargo install --path .
```

## Configuration

Rustagent looks for configuration files in the following locations (in order):

1. `./rustagent.toml` (current directory)
2. `~/.config/rustagent/config.toml` (XDG config directory)
3. `~/.rustagent/config.toml` (home directory)

### Create Configuration File

Copy the example configuration and customize it:

```bash
cp rustagent.toml.example rustagent.toml
```

### Configuration Format

```toml
# LLM Configuration (required)
[llm]
provider = "anthropic"  # Options: "anthropic", "openai", "ollama"
model = "claude-sonnet-4-20250514"
max_tokens = 8192       # Maximum tokens per response

# Provider-specific configuration
[anthropic]
api_key = "${ANTHROPIC_API_KEY}"  # Environment variable substitution

# Agent settings (required)
[rustagent]
spec_dir = "specs"       # Directory for specification files
max_iterations = 100     # Optional: limit execution iterations

# Security settings (optional, has safe defaults)
[security]
shell_policy = "allowlist"  # "allowlist", "blocklist", or "unrestricted"
allowed_commands = ["git", "cargo", "npm", "ls", "cat", "grep", "find"]
max_file_size_mb = 10
allowed_paths = ["."]
```

### Environment Variables

The config system supports environment variable substitution using `${VAR_NAME}` syntax:

```bash
# Set your API key
export ANTHROPIC_API_KEY="your-api-key-here"

# Or for OpenAI
export OPENAI_API_KEY="your-api-key-here"
```

### Provider Options

**Anthropic (Claude)**
```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

[anthropic]
api_key = "${ANTHROPIC_API_KEY}"
```

**OpenAI (GPT)**
```toml
[llm]
provider = "openai"
model = "gpt-4"

[openai]
api_key = "${OPENAI_API_KEY}"
```

**Ollama (Local)**
```toml
[llm]
provider = "ollama"
model = "llama2"

[ollama]
base_url = "http://localhost:11434"
```

### Mode-Specific LLM Overrides

Use different models for planning vs execution:

```toml
# Default LLM
[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"

# Use a more capable model for planning
[planning.llm]
provider = "anthropic"
model = "claude-opus-4-20250514"
max_tokens = 16384

# Use a faster model for Ralph execution
[ralph.llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096
```

### Security Configuration

Control what operations the agent can perform:

```toml
[security]
# Shell policy options:
#   "allowlist"    - Only listed commands can run (default, safest)
#   "blocklist"    - All except blocked patterns can run
#   "unrestricted" - All commands can run
shell_policy = "allowlist"

# Commands allowed when using allowlist policy
allowed_commands = ["git", "cargo", "npm", "ls", "cat", "grep", "find", "echo", "pwd", "mkdir", "touch"]

# Regex patterns to block when using blocklist policy
blocked_patterns = ["rm\\s+-rf", "sudo"]

# Maximum file size the agent can write (MB)
max_file_size_mb = 10

# Paths the agent can access (relative or absolute)
allowed_paths = ["."]
```

## Usage

Rustagent provides four commands: `init`, `plan`, `run`, and `tui`. Running `rustagent` without a command launches the TUI by default.

### Interactive TUI

Launch the terminal user interface:

```bash
rustagent tui
# or simply
rustagent
```

The TUI provides:
- Spec browsing and management
- Real-time agent execution monitoring
- Interactive spec creation and editing

### 1. Initialize a Specification

Create a new agent specification interactively:

```bash
rustagent init --spec-dir ./specs
```

This command (when implemented) will help you create the initial spec structure.

### 2. Plan: Generate Execution Tasks

Convert your specification into an execution plan using the Planning Agent:

```bash
rustagent plan --spec-dir ./specs
```

The Planning Agent will:
- Read your high-level specification
- Break it down into concrete, executable tasks
- Add acceptance criteria for each task
- Save the plan as `spec.json` in your spec directory

### 3. Run: Execute the Plan

Execute the generated plan using the Ralph Loop:

```bash
rustagent run specs/spec.json
```

With iteration limit:

```bash
rustagent run specs/spec.json --max-iterations 10
```

The Ralph Loop will:
- Read the next pending task
- Act using available tools to complete the task
- Learn from the results and update the spec
- Plan the next action based on learnings
- Ask for help if blocked (human intervention required)
- Continue until all tasks are complete or max iterations reached

## How It Works

### Planning Agent

The Planning Agent transforms high-level goals into structured execution plans:

1. **Input**: A specification directory containing your requirements
2. **Process**: LLM-powered analysis and task decomposition
3. **Output**: A `spec.json` file with structured tasks

Example flow:
```
User Spec → Planning Agent → Structured Tasks → spec.json
```

### Ralph Loop (Read-Act-Learn-Plan-Help)

The Ralph Loop is the core execution engine:

1. **Read**: Load the spec and find the next pending task
2. **Act**: Use tools (file operations, shell commands) to work on the task
3. **Learn**: Analyze results and update learnings in the spec
4. **Plan**: Determine the next action based on context
5. **Help**: Ask for human intervention when blocked

The loop continues until:
- All tasks are complete
- A task is blocked (requires human help)
- Maximum iterations reached (if configured)

## Specification Format

Rustagent uses a JSON-based specification format:

```json
{
  "name": "implement-user-auth",
  "description": "Implement user authentication system with JWT",
  "branch_name": "feature/user-auth",
  "created_at": "2026-01-20T10:30:00Z",
  "tasks": [
    {
      "id": "task-1",
      "title": "Create User model",
      "description": "Define User struct with fields for authentication",
      "acceptance_criteria": [
        "User struct has email, password_hash, and created_at fields",
        "Implements proper serialization/deserialization",
        "Includes validation for email format"
      ],
      "status": "pending",
      "blocked_reason": null,
      "completed_at": null
    }
  ],
  "learnings": []
}
```

### Task Statuses

- `pending`: Task is ready to be worked on
- `in_progress`: Task is currently being executed
- `complete`: Task has been successfully completed
- `blocked`: Task cannot proceed without human intervention

## Available Tools

Rustagent provides the following tools to agents:

### File Operations

- **read_file**: Read contents of a file
  ```json
  {
    "name": "read_file",
    "arguments": {
      "path": "src/main.rs"
    }
  }
  ```

- **write_file**: Write or overwrite file contents
  ```json
  {
    "name": "write_file",
    "arguments": {
      "path": "src/config.rs",
      "content": "// File contents here"
    }
  }
  ```

- **list_files**: List files in a directory
  ```json
  {
    "name": "list_files",
    "arguments": {
      "path": "src",
      "recursive": false
    }
  }
  ```

### Shell Commands

- **run_command**: Execute shell commands
  ```json
  {
    "name": "run_command",
    "arguments": {
      "command": "cargo test",
      "working_dir": "."
    }
  }
  ```

### Task Completion

- **signal_completion**: Signal task completion or blocked status
  ```json
  {
    "name": "signal_completion",
    "arguments": {
      "signal": "complete",
      "message": "Task finished successfully"
    }
  }
  ```

  Or to signal blocked:
  ```json
  {
    "name": "signal_completion",
    "arguments": {
      "signal": "blocked",
      "reason": "Missing required dependency"
    }
  }
  ```

## Development

### Building

```bash
# Debug build
cargo build

# Release build with optimizations
cargo build --release

# Quick compile check
cargo check
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy
```

### Testing

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test

# Run specific test
cargo test test_name
```

### Logging

Rustagent uses `tracing` for file-based logging. Logs are stored in:

- `$XDG_STATE_HOME/rustagent/logs/` (if XDG_STATE_HOME is set)
- `~/.local/share/rustagent/logs/` (macOS/Linux)
- `~/.local/state/rustagent/logs/` (fallback)

Logs are rotated daily. To adjust the log level, set the `RUST_LOG` environment variable:

```bash
RUST_LOG=rustagent=debug cargo run -- plan
```

### Documentation

```bash
# Generate and view documentation
cargo doc --open
```

### Project Structure

```
rustagent/
├── src/
│   ├── main.rs           # CLI entry point with init/plan/run/tui commands
│   ├── lib.rs            # Library exports
│   ├── config.rs         # Configuration loading with env var substitution
│   ├── logging.rs        # File-based tracing with daily rotation
│   ├── spec.rs           # Specification data structures
│   ├── llm/
│   │   ├── mod.rs        # LlmClient trait and Message types
│   │   ├── anthropic.rs  # Anthropic (Claude) client
│   │   ├── openai.rs     # OpenAI (GPT) client
│   │   ├── ollama.rs     # Ollama (local models) client
│   │   ├── mock.rs       # Mock client for testing
│   │   └── factory.rs    # Client factory based on config
│   ├── planning/
│   │   └── mod.rs        # Planning Agent implementation
│   ├── ralph/
│   │   └── mod.rs        # Ralph Loop execution engine
│   ├── security/
│   │   ├── mod.rs        # Security validator for paths/commands
│   │   └── permission.rs # Permission handling (CLI prompts)
│   ├── tools/
│   │   ├── mod.rs        # Tool trait and registry
│   │   ├── file.rs       # read_file, write_file, list_files tools
│   │   ├── shell.rs      # run_command tool
│   │   ├── signal.rs     # signal_completion tool
│   │   ├── factory.rs    # Tool registry factory
│   │   └── permission_check.rs  # File permission checking
│   └── tui/
│       ├── mod.rs        # TUI module exports and terminal setup
│       ├── app.rs        # Application state and event handling
│       ├── ui.rs         # UI rendering logic
│       ├── messages.rs   # Agent-TUI message channel
│       ├── views/        # View components (spec list, detail, etc.)
│       └── widgets/      # Reusable TUI widgets
├── specs/                # Default directory for specifications
├── tests/                # Integration tests
├── Cargo.toml           # Package manifest
└── rustagent.toml       # Configuration file (create from example)
```

## Architecture

Rustagent follows a modular architecture:

- **Config System**: Multi-provider LLM configuration with environment variable support
- **LLM Clients**: Abstracted clients for different LLM providers (streaming support)
- **Tool System**: Trait-based tool implementation with dynamic registry (`Arc<RwLock>` for thread safety)
- **Security Layer**: Path and command validation with interactive permission prompts
- **Spec Management**: JSON persistence layer for task tracking and learning
- **TUI**: ratatui-based terminal interface with async agent communication via channels
- **Agents**: Planning Agent and Ralph Loop for two-phase execution

## Examples

### Example 1: Create a New Feature

1. Create a specification:
```bash
mkdir -p specs/add-logging
echo '{"description": "Add structured logging to the application"}' > specs/add-logging/description.txt
```

2. Generate execution plan:
```bash
rustagent plan --spec-dir specs/add-logging
```

3. Execute the plan:
```bash
rustagent run specs/add-logging/spec.json
```

### Example 2: Limited Iteration Run

Run with a safety limit on iterations:

```bash
rustagent run specs/spec.json --max-iterations 5
```

This will stop after 5 iterations, even if tasks remain pending.

## References

- **Rust Edition 2024**: This project uses Rust edition 2024, stable since Rust 1.85.0
- **Version Control**: Compatible with both Git and Jujutsu
- **Anthropic API**: https://docs.anthropic.com/
- **OpenAI API**: https://platform.openai.com/docs/
- **Ollama**: https://ollama.ai/

## Contributing

Contributions are welcome! This is an exploration project for implementing AI agents in Rust.

## License

See LICENSE file for details.
