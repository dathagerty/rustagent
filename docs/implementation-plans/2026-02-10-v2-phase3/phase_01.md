# Rustagent V2 Phase 3a: Dependencies + Daemon Lifecycle

**Goal:** Add HTTP server dependencies and implement the daemon lifecycle — start, stop, PID file management, health check — plus the `daemon` CLI subcommand.

**Architecture:** The daemon is the same Rust binary with a `daemon` subcommand. It starts an axum HTTP server on a configurable port (default `127.0.0.1:7400`), writes a PID file to `~/.local/share/rustagent/rustagent.pid`, and manages orchestrators for active goals. The daemon is a single long-running process; the CLI auto-detects it via PID file + health check.

**Tech Stack:** Rust (edition 2024), axum 0.8, tower 0.5, tower-http 0.6, tokio 1.43, clap 4.5 (derive)

**Scope:** Phase 1 of 7 from the v2 Phase 3 architecture (Daemon + HTTP API)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and tests:

### P3a.AC1: New dependencies compile
- **P3a.AC1.1 Success:** `axum` 0.8 with `ws` feature added to Cargo.toml and compiles
- **P3a.AC1.2 Success:** `tower` 0.5 added and compiles
- **P3a.AC1.3 Success:** `tower-http` 0.6 with `cors` and `fs` features added and compiles
- **P3a.AC1.4 Success:** `rust-embed` 8 with `axum` feature added as optional dependency behind `bundle-ui` feature flag

### P3a.AC2: DaemonConfig
- **P3a.AC2.1 Success:** `DaemonConfig` struct has `bind_address` (default `127.0.0.1`), `port` (default `7400`), `pid_file` (default XDG data dir), `log_dir` (default XDG state dir)
- **P3a.AC2.2 Success:** `DaemonConfig::default()` returns documented defaults
- **P3a.AC2.3 Success:** `DaemonConfig::socket_addr()` returns a valid `SocketAddr`

### P3a.AC3: PID file management
- **P3a.AC3.1 Success:** `write_pid_file()` writes current process PID to the configured path, creating parent directories if needed
- **P3a.AC3.2 Success:** `read_pid_file()` reads and returns the PID from the file, or None if the file doesn't exist
- **P3a.AC3.3 Success:** `remove_pid_file()` deletes the PID file
- **P3a.AC3.4 Success:** `is_daemon_running()` returns true if PID file exists AND the process is alive (kill(pid, 0) check on Unix)
- **P3a.AC3.5 Success:** `is_daemon_running()` returns false if PID file exists but the process is dead (stale PID file)

### P3a.AC4: Daemon CLI subcommand
- **P3a.AC4.1 Success:** `rustagent daemon start` starts the daemon in the foreground (background daemonization deferred — use systemd/launchd for background)
- **P3a.AC4.2 Success:** `rustagent daemon stop` reads PID file and sends SIGTERM to the daemon process
- **P3a.AC4.3 Success:** `rustagent daemon status` reports whether the daemon is running (PID file check + process alive check)
- **P3a.AC4.4 Success:** `rustagent daemon start` prints an error if a daemon is already running
- **P3a.AC4.5 Success:** `rustagent daemon stop` prints an error if no daemon is running
- **P3a.AC4.6 Success:** `rustagent daemon logs` tails the daemon log directory (defaults to `~/.local/state/rustagent/logs/`)

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Add HTTP server dependencies to Cargo.toml

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/Cargo.toml`

**Implementation:**

Add the following to `[dependencies]` after the existing `glob` entry:

```toml
axum = { version = "0.8", features = ["ws"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }
# Note: Architecture doc specifies features = ["cors", "fs"] but "fs" is intentionally omitted —
# Phase 3g uses rust-embed for static file serving instead of tower-http's fs module.
```

Add an optional dependency for the bundle-ui feature:

```toml
rust-embed = { version = "8", features = ["axum"], optional = true }
```

Add a `[features]` section:

```toml
[features]
bundle-ui = ["dep:rust-embed"]
```

**Verification:**

Run: `cargo check`
Expected: Compiles without errors

**Commit:** `chore: add axum, tower, tower-http dependencies for daemon HTTP server`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create daemon module with lifecycle management

**Verifies:** P3a.AC2.1, P3a.AC2.2, P3a.AC2.3, P3a.AC3.1, P3a.AC3.2, P3a.AC3.3, P3a.AC3.4, P3a.AC3.5

**Files:**
- Create: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/daemon/mod.rs`
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/lib.rs` — add `pub mod daemon;`
- Test: `/Users/david.hagerty/code/personal/rustagent/new-directions/tests/daemon_test.rs`

**Implementation:**

`src/daemon/mod.rs` is the daemon module root. It re-exports submodules (added in later phases) and contains the daemon lifecycle types.

Submodule declarations (most will be created in later phases):

```rust
pub mod server;   // Phase 3b
pub mod api;      // Phase 3b
pub mod ws;       // Phase 3e
```

For now, only `mod.rs` is created; submodule declarations are commented out until their files exist.

**1. DaemonConfig:**

```rust
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub bind_address: String,   // Default: "127.0.0.1"
    pub port: u16,              // Default: 7400
    pub pid_file: PathBuf,      // Default: ~/.local/share/rustagent/rustagent.pid
    pub log_dir: PathBuf,       // Default: ~/.local/state/rustagent/logs/
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustagent");
        let state_dir = dirs::state_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustagent")
            .join("logs");

        Self {
            bind_address: "127.0.0.1".to_string(),
            port: 7400,
            pid_file: data_dir.join("rustagent.pid"),
            log_dir: state_dir,
        }
    }
}

impl DaemonConfig {
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        format!("{}:{}", self.bind_address, self.port).parse()
    }
}
```

**2. PID file management:**

```rust
use std::fs;
use std::io;

/// Write the current process PID to the PID file
pub fn write_pid_file(config: &DaemonConfig) -> anyhow::Result<()> {
    if let Some(parent) = config.pid_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config.pid_file, std::process::id().to_string())?;
    Ok(())
}

/// Read the PID from the PID file, or None if it doesn't exist
pub fn read_pid_file(config: &DaemonConfig) -> anyhow::Result<Option<u32>> {
    match fs::read_to_string(&config.pid_file) {
        Ok(content) => {
            let pid: u32 = content.trim().parse()?;
            Ok(Some(pid))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Remove the PID file
pub fn remove_pid_file(config: &DaemonConfig) -> anyhow::Result<()> {
    match fs::remove_file(&config.pid_file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Check if the daemon is running by reading PID file and checking process liveness
pub fn is_daemon_running(config: &DaemonConfig) -> anyhow::Result<bool> {
    match read_pid_file(config)? {
        Some(pid) => {
            // On Unix, kill(pid, 0) checks if process exists without sending a signal
            #[cfg(unix)]
            {
                let result = unsafe { libc::kill(pid as i32, 0) };
                Ok(result == 0)
            }
            #[cfg(not(unix))]
            {
                // Fallback: assume running if PID file exists
                Ok(true)
            }
        }
        None => Ok(false),
    }
}
```

Note: Add `libc` as a dependency for Unix process checking:

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

**Testing:**

Tests in `tests/daemon_test.rs`:

- P3a.AC2.1: Construct `DaemonConfig::default()`, verify all fields have documented defaults
- P3a.AC2.2: Same as AC2.1
- P3a.AC2.3: `DaemonConfig::default().socket_addr()` returns `Ok(127.0.0.1:7400)`
- P3a.AC3.1: Create a config with a temp dir PID path. Call `write_pid_file`. Verify file exists and contains the current PID.
- P3a.AC3.2: Write a PID file, then `read_pid_file` returns `Some(pid)`. Also test with nonexistent file returns `None`.
- P3a.AC3.3: Write a PID file, `remove_pid_file`, verify file no longer exists. Also test removing nonexistent file succeeds.
- P3a.AC3.4: Write current process PID. `is_daemon_running` returns true (our own PID is alive).
- P3a.AC3.5: Write a PID of 99999999 (almost certainly dead). `is_daemon_running` returns false.

```rust
use rustagent::daemon::*;
use tempfile::TempDir;

#[test]
fn test_daemon_config_defaults() {
    let config = DaemonConfig::default();
    assert_eq!(config.bind_address, "127.0.0.1");
    assert_eq!(config.port, 7400);
    assert!(config.pid_file.to_string_lossy().contains("rustagent.pid"));
}

#[test]
fn test_socket_addr() {
    let config = DaemonConfig::default();
    let addr = config.socket_addr().unwrap();
    assert_eq!(addr.port(), 7400);
}

#[test]
fn test_pid_file_write_read_remove() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };

    write_pid_file(&config).unwrap();
    let pid = read_pid_file(&config).unwrap();
    assert_eq!(pid, Some(std::process::id()));

    remove_pid_file(&config).unwrap();
    assert_eq!(read_pid_file(&config).unwrap(), None);
}

#[test]
fn test_is_daemon_running_with_live_pid() {
    let tmp = TempDir::new().unwrap();
    let config = DaemonConfig {
        pid_file: tmp.path().join("test.pid"),
        ..DaemonConfig::default()
    };
    write_pid_file(&config).unwrap();
    assert!(is_daemon_running(&config).unwrap());
}
```

**Verification:**

Run: `cargo test daemon_test`
Expected: All tests pass

**Commit:** `feat(daemon): DaemonConfig and PID file lifecycle management`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add daemon CLI subcommand

**Verifies:** P3a.AC4.1, P3a.AC4.2, P3a.AC4.3, P3a.AC4.4, P3a.AC4.5

**Files:**
- Modify: `/Users/david.hagerty/code/personal/rustagent/new-directions/src/main.rs` — add `Daemon` command variant and `DaemonAction` subcommands

**Implementation:**

Add to the `Commands` enum:

```rust
/// Manage the daemon process
Daemon {
    #[command(subcommand)]
    action: DaemonAction,
},
```

Add the `DaemonAction` enum:

```rust
#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon (foreground)
    Start {
        /// Bind address (default: 127.0.0.1)
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// Port (default: 7400)
        #[arg(long, default_value = "7400")]
        port: u16,
    },
    /// Stop a running daemon
    Stop,
    /// Check if the daemon is running
    Status,
    /// Tail daemon logs
    Logs {
        /// Number of lines to show (default: 50)
        #[arg(long, short = 'n', default_value = "50")]
        lines: usize,
        /// Follow log output (like tail -f)
        #[arg(long, short = 'f')]
        follow: bool,
    },
}
```

Add the match arm for `Commands::Daemon`:

```rust
Commands::Daemon { action } => {
    let config = rustagent::daemon::DaemonConfig::default();

    match action {
        DaemonAction::Start { bind, port } => {
            let config = rustagent::daemon::DaemonConfig {
                bind_address: bind,
                port,
                ..config
            };

            // Check if already running
            if rustagent::daemon::is_daemon_running(&config)? {
                anyhow::bail!("Daemon is already running (PID file: {})",
                    config.pid_file.display());
            }

            // Write PID file
            rustagent::daemon::write_pid_file(&config)?;

            // Set up cleanup on exit
            let cleanup_config = config.clone();
            let shutdown_token = tokio_util::sync::CancellationToken::new();
            let shutdown_clone = shutdown_token.clone();

            tokio::spawn(async move {
                tokio::signal::ctrl_c().await.ok();
                println!("\nDaemon shutting down...");
                shutdown_clone.cancel();
            });

            println!("Daemon starting on {}:{}", config.bind_address, config.port);
            println!("PID file: {}", config.pid_file.display());

            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;

            // Start the HTTP server (Phase 3b will implement this)
            // For now, just wait for shutdown
            shutdown_token.cancelled().await;

            // Cleanup
            rustagent::daemon::remove_pid_file(&cleanup_config)?;
            println!("Daemon stopped.");
        }
        DaemonAction::Stop => {
            match rustagent::daemon::read_pid_file(&config)? {
                Some(pid) => {
                    if !rustagent::daemon::is_daemon_running(&config)? {
                        println!("Stale PID file (process {} not running). Cleaning up.", pid);
                        rustagent::daemon::remove_pid_file(&config)?;
                        return Ok(());
                    }

                    println!("Stopping daemon (PID {})...", pid);
                    #[cfg(unix)]
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                    println!("Signal sent. Daemon should stop shortly.");
                }
                None => {
                    println!("No daemon is running (no PID file found).");
                }
            }
        }
        DaemonAction::Status => {
            if rustagent::daemon::is_daemon_running(&config)? {
                let pid = rustagent::daemon::read_pid_file(&config)?.unwrap();
                println!("Daemon is running (PID {})", pid);
                println!("  Address: {}:{}", config.bind_address, config.port);
                println!("  PID file: {}", config.pid_file.display());
            } else {
                println!("Daemon is not running.");
                if config.pid_file.exists() {
                    println!("  (stale PID file at {})", config.pid_file.display());
                }
            }
        }
        DaemonAction::Logs { lines, follow } => {
            let log_dir = &config.log_dir;
            if !log_dir.exists() {
                println!("No log directory found at {}", log_dir.display());
                return Ok(());
            }

            // Find the most recent log file in the log directory
            let mut entries: Vec<_> = std::fs::read_dir(log_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "log"))
                .collect();
            entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));

            if entries.is_empty() {
                println!("No log files found in {}", log_dir.display());
                return Ok(());
            }

            let log_file = entries[0].path();
            println!("Tailing {}", log_file.display());

            if follow {
                // Use tail -f for follow mode (simple and correct)
                let status = std::process::Command::new("tail")
                    .args(["-n", &lines.to_string(), "-f"])
                    .arg(&log_file)
                    .status()?;
                std::process::exit(status.code().unwrap_or(1));
            } else {
                let content = std::fs::read_to_string(&log_file)?;
                let all_lines: Vec<&str> = content.lines().collect();
                let start = all_lines.len().saturating_sub(lines);
                for line in &all_lines[start..] {
                    println!("{}", line);
                }
            }
        }
    }
}
```

Note: Add `libc` to `[target.'cfg(unix)'.dependencies]` in Cargo.toml (if not already done in Task 2).

**Verification:**

Run: `cargo build`
Expected: Compiles cleanly

Run: `cargo run -- daemon --help`
Expected: Shows start, stop, status subcommands

Run: `cargo run -- daemon status`
Expected: "Daemon is not running."

**Commit:** `feat(cli): add daemon subcommand with start, stop, and status`

<!-- END_TASK_3 -->
<!-- END_SUBCOMPONENT_A -->
