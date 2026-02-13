# V2 Phase 5 - Security Scope Enforcement

**Goal:** Add enforcement logic to `SecurityScope` so that per-agent security boundaries are checked at tool execution time. Currently `SecurityScope` is a pure data struct with no validation methods.

**Architecture:** Add `check_path` and `check_command` methods to `SecurityScope` that use glob pattern matching (via the `glob` crate's `Pattern::matches_path`) to validate file paths and shell commands against the scope's allowed/denied lists. The `read_only` and `can_create_files` flags are checked separately. Tool implementations (`ReadFileTool`, `WriteFileTool`, `RunCommandTool`) will use these methods in Phase 5 (wiring), but this phase focuses on the SecurityScope logic itself.

**Tech Stack:** Rust, `glob` 0.3 (already in Cargo.toml)

**Scope:** 1 of 6 phases from original design (Phase 5, item 4)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC7: Path Validation
- **v2-phase5.AC7.1 Success:** `check_path` allows a path matching an allowed pattern (e.g., `src/main.rs` matches `src/**`)
- **v2-phase5.AC7.2 Success:** `check_path` denies a path matching a denied pattern even if it also matches an allowed pattern (deny takes precedence)
- **v2-phase5.AC7.3 Success:** `check_path` denies all write operations when `read_only` is true
- **v2-phase5.AC7.4 Success:** `check_path` denies creating new files when `can_create_files` is false (but allows editing existing files)
- **v2-phase5.AC7.5 Success:** Wildcard `*` in allowed_paths matches everything

### v2-phase5.AC8: Command Validation
- **v2-phase5.AC8.1 Success:** `check_command` allows a command matching an allowed pattern (e.g., `cargo test` matches `cargo *`)
- **v2-phase5.AC8.2 Success:** `check_command` denies a command not matching any allowed pattern
- **v2-phase5.AC8.3 Success:** Wildcard `*` in allowed_commands matches everything

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Implement check_path on SecurityScope

**Verifies:** v2-phase5.AC7.1, v2-phase5.AC7.2, v2-phase5.AC7.3, v2-phase5.AC7.4, v2-phase5.AC7.5

**Files:**
- Modify: `src/security/scope.rs` (add methods)

**Implementation:**

Add an enum and methods to `SecurityScope`:

```rust
use glob::{MatchOptions, Pattern};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Read,
    Write,
    Create,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCheck {
    Allowed,
    Denied(String),
}

/// Match options that allow `**` to match across directory separators.
fn glob_match_options() -> MatchOptions {
    MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,  // allows `**` to match `/`
        require_literal_leading_dot: false,
    }
}

impl SecurityScope {
    /// Check whether a file operation is permitted for the given path.
    pub fn check_path(&self, path: &str, operation: FileOperation) -> ScopeCheck {
        // Check read_only constraint
        if self.read_only && matches!(operation, FileOperation::Write | FileOperation::Create) {
            return ScopeCheck::Denied("read-only scope".to_string());
        }

        // Check can_create_files constraint
        if !self.can_create_files && operation == FileOperation::Create {
            return ScopeCheck::Denied("file creation not allowed".to_string());
        }

        let opts = glob_match_options();
        let path_ref = Path::new(path);

        // Check denied patterns first (deny takes precedence)
        for pattern_str in &self.denied_paths {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches_path_with(path_ref, opts) {
                    return ScopeCheck::Denied(format!("path matches denied pattern: {}", pattern_str));
                }
            }
        }

        // Check allowed patterns
        for pattern_str in &self.allowed_paths {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches_path_with(path_ref, opts) {
                    return ScopeCheck::Allowed;
                }
            }
        }

        ScopeCheck::Denied("path not in allowed patterns".to_string())
    }
}
```

**Important:** `Pattern::matches_path_with` with `require_literal_separator: false` allows `**` and `*` to match across `/` directory separators, so `src/**` correctly matches `src/auth/handler.rs`. Without this option, `*` would not match `/` and recursive patterns would fail.

**Testing:**
Tests must verify each AC case listed above. Use inline `#[cfg(test)]` module in `scope.rs`. Must include a test that `src/**` matches `src/auth/handler.rs` (recursive directory matching).

**Verification:**
Run: `cargo test scope`
Expected: All tests pass

**Commit:** `feat(security): add check_path to SecurityScope`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Implement check_command on SecurityScope

**Verifies:** v2-phase5.AC8.1, v2-phase5.AC8.2, v2-phase5.AC8.3

**Files:**
- Modify: `src/security/scope.rs` (add `check_command` method)

**Implementation:**

Add to `SecurityScope`:

```rust
impl SecurityScope {
    /// Check whether a shell command is permitted.
    pub fn check_command(&self, command: &str) -> ScopeCheck {
        for pattern_str in &self.allowed_commands {
            if let Ok(pattern) = Pattern::new(pattern_str) {
                if pattern.matches(command) {
                    return ScopeCheck::Allowed;
                }
            }
        }

        ScopeCheck::Denied(format!("command not in allowed patterns: {}", command))
    }

    /// Check whether network access is permitted.
    pub fn check_network(&self) -> ScopeCheck {
        if self.network_access {
            ScopeCheck::Allowed
        } else {
            ScopeCheck::Denied("network access not allowed".to_string())
        }
    }
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC8.1: `cargo test` matches `cargo *`
- v2-phase5.AC8.2: `rm -rf /` does not match `cargo *`
- v2-phase5.AC8.3: any command matches `*`

**Verification:**
Run: `cargo test scope`
Expected: All tests pass

**Commit:** `feat(security): add check_command and check_network to SecurityScope`
<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Integration tests for SecurityScope with built-in profiles

**Verifies:** v2-phase5.AC7.1, v2-phase5.AC7.3, v2-phase5.AC8.1

**Files:**
- Create: `tests/security_scope_test.rs`

**Implementation:**

Write integration tests that verify the built-in profile security scopes work correctly:

1. Load the `planner()` profile from `src/agent/builtin_profiles.rs` and verify its `SecurityScope` denies writes (`read_only: true`)
2. Load the `coder()` profile and verify its scope allows file writes within project paths
3. Load the `reviewer()` profile and verify its scope denies writes (`read_only: true`)
4. Load the `researcher()` profile and verify its scope denies writes (`read_only: true`)

These tests import from `rustagent::agent::builtin_profiles` and call `check_path`/`check_command` on each profile's security scope.

**Testing:**
These ARE the tests. They verify that built-in profiles integrate correctly with the new enforcement methods.

**Verification:**
Run: `cargo test security_scope`
Expected: All tests pass

**Commit:** `test(security): integration tests for SecurityScope with built-in profiles`
<!-- END_TASK_3 -->

<!-- END_SUBCOMPONENT_A -->
