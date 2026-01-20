# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustagent is an exploration project for implementing an AI agent in Rust. The project is currently in early development stages with placeholder code.

## Development Commands

### Building and Running
```bash
cargo build              # Compile in debug mode
cargo build --release    # Compile optimized release build
cargo run               # Build and run the application
cargo check             # Fast compilation check without producing binary
```

### Code Quality
```bash
cargo fmt               # Format code using rustfmt
cargo clippy            # Run Clippy linter for code improvements
```

### Testing
```bash
cargo test              # Run test suite (when tests are added)
```

### Documentation
```bash
cargo doc --open        # Generate and view documentation
```

## Project Structure

Currently minimal with a single entry point:
- `src/main.rs` - Main application entry point (currently "Hello, world!" placeholder)
- `Cargo.toml` - Package manifest with no external dependencies yet

## Important Notes

### Cargo Edition
The `Cargo.toml` specifies `edition = "2024"`, which is the recommended edition for new projects. Rust editions are backward-compatible milestones that include opt-in changes. Valid editions are 2015, 2018, 2021, and 2024 (stable since Rust 1.85.0 in February 2025).

### Version Control
This repository uses both Git and Jujutsu (`.jj/` directory present). Be aware of this dual VCS setup when making version control operations.

### Architecture
No architectural patterns or frameworks have been established yet. This is a greenfield project where design decisions for AI agent implementation are still to be made, including:
- Choice of AI/LLM libraries
- Agent architecture patterns
- State management approach
- External dependencies
