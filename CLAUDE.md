# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustagent is a Rust-based AI agent framework for autonomous task execution. It uses a two-phase approach: a Planning Agent that breaks down high-level goals into executable tasks, and the Ralph Loop that executes those tasks iteratively with tool access.

## Development Commands

### Building and Running
```bash
cargo build              # Compile in debug mode
cargo build --release    # Compile optimized release build
cargo run -- init        # Initialize a new spec directory
cargo run -- plan        # Run the planning agent
cargo run -- run <spec>  # Execute a spec with the Ralph loop
cargo check              # Fast compilation check without producing binary
```

### Code Quality
```bash
cargo fmt               # Format code using rustfmt
cargo clippy            # Run Clippy linter for code improvements
```

### Testing
```bash
cargo test              # Run test suite
cargo test <name>       # Run specific test by name
```

### Documentation
```bash
cargo doc --open        # Generate and view documentation
```

## Project Structure

```
src/
├── main.rs           # CLI entry point with init/plan/run commands
├── lib.rs            # Library exports
├── config.rs         # Configuration loading with env var substitution
├── logging.rs        # File-based tracing with daily rotation
├── spec.rs           # Specification data structures
├── llm/
│   ├── mod.rs        # LlmClient trait and Message types
│   ├── anthropic.rs  # Anthropic (Claude) client
│   ├── openai.rs     # OpenAI (GPT) client
│   ├── ollama.rs     # Ollama (local models) client
│   ├── mock.rs       # Mock client for testing
│   └── factory.rs    # Client factory based on config
├── planning/
│   └── mod.rs        # Planning Agent implementation
├── ralph/
│   └── mod.rs        # Ralph Loop execution engine
├── security/
│   ├── mod.rs        # Security validator for paths/commands
│   └── permission.rs # Permission handling (CLI prompts)
└── tools/
    ├── mod.rs        # Tool trait and registry
    ├── file.rs       # read_file, write_file, list_files tools
    ├── shell.rs      # run_command tool
    ├── signal.rs     # signal_completion tool
    ├── factory.rs    # Tool registry factory
    └── permission_check.rs  # File permission checking
```

## Important Notes

### Cargo Edition
The `Cargo.toml` specifies `edition = "2024"`, which requires Rust 1.85.0 or later. This is the recommended edition for new Rust projects as of 2025.

### LLM Providers
The project supports three LLM providers:
- **Anthropic** (Claude) - Default, uses tool calling API
- **OpenAI** (GPT-4) - Full tool calling with tool_call_id
- **Ollama** (Local) - For running local models

### Logging
Logs are written to `~/.local/state/rustagent/logs/` with daily rotation. Set `RUST_LOG=rustagent=debug` for verbose output.

### Version Control
This repository uses both Git and Jujutsu (`.jj/` directory present). Be aware of this dual VCS setup when making version control operations.

### Architecture Patterns
- **Factory pattern**: Used for LLM clients and tool registry
- **Trait objects**: `dyn LlmClient` and `dyn Tool` for runtime polymorphism
- **Arc/RwLock**: Thread-safe shared state in tool registry
- **anyhow::Result**: Unified error handling across the codebase
