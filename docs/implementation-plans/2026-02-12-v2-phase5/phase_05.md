# V2 Phase 5 - Code Search Tool

**Goal:** Build a file-content search tool that agents can use to search project source code. This is distinct from the existing `search_nodes` graph tool — this searches file contents on disk, like grep.

**Architecture:** A new `src/tools/search.rs` module implements the `Tool` trait. It uses `walkdir` for directory traversal and `regex` for pattern matching. The tool accepts a search pattern, optional file glob filter, optional directory scope, and returns matching lines with file paths and line numbers. Results are capped to prevent flooding the agent's context. The tool respects the agent's `SecurityScope` path restrictions.

**Tech Stack:** Rust, `walkdir` 2 (already in Cargo.toml), `regex` 1.10 (already in Cargo.toml)

**Scope:** 1 of 6 phases from original design (Phase 5, item 5)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC9: Code Search Functionality
- **v2-phase5.AC9.1 Success:** Searching for a known string pattern returns matching lines with file paths and line numbers
- **v2-phase5.AC9.2 Success:** File glob filter limits search to matching files (e.g., `*.rs` searches only Rust files)
- **v2-phase5.AC9.3 Success:** Results are capped at a configurable limit (default 50 matches) to avoid flooding agent context
- **v2-phase5.AC9.4 Success:** Binary files are skipped (files that fail UTF-8 decoding)

### v2-phase5.AC10: Code Search Registration
- **v2-phase5.AC10.1 Success:** The search tool is registered in the V2 tool registry and available to agents
- **v2-phase5.AC10.2 Success:** The tool's JSON schema describes `pattern` (required), `file_glob` (optional), `directory` (optional), and `max_results` (optional) parameters

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Implement CodeSearchTool

**Verifies:** v2-phase5.AC9.1, v2-phase5.AC9.2, v2-phase5.AC9.3, v2-phase5.AC9.4, v2-phase5.AC10.2

**Files:**
- Create: `src/tools/search.rs`
- Modify: `src/tools/mod.rs` (add `pub mod search;`)

**Implementation:**

Create `src/tools/search.rs` implementing the `Tool` trait:

```rust
use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use glob::Pattern;
use regex::Regex;
use serde_json::json;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct CodeSearchTool {
    project_root: PathBuf,
}

impl CodeSearchTool {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}
```

The `execute` method should:
1. Parse parameters: `pattern` (required string — used as regex), `file_glob` (optional string — glob pattern for filenames), `directory` (optional string — subdirectory to scope search), `max_results` (optional number, default 50)
2. Compile the regex pattern (return error if invalid)
3. If `directory` is provided, scope the walk to `project_root/directory`; otherwise walk from `project_root`
4. Use `WalkDir` to iterate files, skipping hidden directories (`.git`, `.jj`, `node_modules`, `target`)
5. If `file_glob` is set, use `glob::Pattern::matches` to filter filenames
6. Read each file as UTF-8, skipping files that fail (binary files)
7. For each matching line, format as `{relative_path}:{line_number}: {line_content}`
8. Stop after `max_results` matches
9. Return the formatted results, plus a summary line like `"Found N matches (limited to max_results)"` if the cap was hit

Parameters JSON schema:
```json
{
    "type": "object",
    "properties": {
        "pattern": {
            "type": "string",
            "description": "Regex pattern to search for in file contents"
        },
        "file_glob": {
            "type": "string",
            "description": "Glob pattern to filter files (e.g., '*.rs', '*.ts')"
        },
        "directory": {
            "type": "string",
            "description": "Subdirectory to scope the search (relative to project root)"
        },
        "max_results": {
            "type": "integer",
            "description": "Maximum number of matching lines to return (default: 50)"
        }
    },
    "required": ["pattern"]
}
```

**Testing:**
Tests must verify:
- v2-phase5.AC9.1: Search finds known pattern, result includes file path and line number
- v2-phase5.AC9.2: File glob filters correctly
- v2-phase5.AC9.3: Results are capped at max_results
- v2-phase5.AC9.4: Binary files don't cause errors

Use tempdir with test files for testing.

**Verification:**
Run: `cargo test code_search`
Expected: All tests pass

Note: Use `code_search` prefix for all test function names in the inline `#[cfg(test)]` module to avoid collision with existing `search_nodes` tests in graph_tools.

**Commit:** `feat(tools): add CodeSearchTool for file content search`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Register CodeSearchTool in the V2 registry

**Verifies:** v2-phase5.AC10.1

**Files:**
- Modify: `src/tools/factory.rs` (add CodeSearchTool to `create_v2_registry`)

**Implementation:**

Add the CodeSearchTool to `create_v2_registry`. The tool needs the project root path, which should be passed as a new parameter to `create_v2_registry`.

1. Add `project_root: PathBuf` parameter to `create_v2_registry`
2. Register the tool: `registry.register(Arc::new(CodeSearchTool::new(project_root)));`
3. Add the import: `use crate::tools::search::CodeSearchTool;`
4. Update all call sites of `create_v2_registry` to pass the project root path

Call sites for `create_v2_registry` (verified via grep):
- `src/agent/orchestrator.rs:821` (in `spawn_worker_with_id`)
- `tests/agent_tools_test.rs:438, 473` (two test functions)
- `tests/graph_tools_test.rs:665` (test function)

Note: `src/main.rs` does NOT call `create_v2_registry` — it uses `create_default_registry` for the CLI. Only the orchestrator and test files need updating.

**Testing:**
No dedicated test needed — the compiler will enforce the new parameter is passed.

**Verification:**
Run: `cargo check`
Expected: Compiles without errors

**Commit:** `feat(tools): register CodeSearchTool in V2 registry`
<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Integration test for CodeSearchTool

**Verifies:** v2-phase5.AC9.1, v2-phase5.AC9.2, v2-phase5.AC9.3

**Files:**
- Create: `tests/code_search_test.rs`

**Implementation:**

Write integration tests using a tempdir with multiple test files:

1. Create a tempdir with:
   - `src/main.rs` containing `fn main() { println!("hello"); }`
   - `src/lib.rs` containing `pub fn add(a: i32, b: i32) -> i32 { a + b }`
   - `README.md` containing `# My Project`
   - A subdirectory `src/utils/helper.rs` containing `pub fn helper() {}`

2. Test cases:
   - Search for `"fn main"` → finds `src/main.rs:1`
   - Search for `"pub fn"` → finds both `.rs` files
   - Search for `"pub fn"` with `file_glob: "*.rs"` → finds `.rs` files, not `README.md`
   - Search with `max_results: 1` → returns exactly 1 result with cap notice
   - Search for `"nonexistent_pattern"` → returns "No matches found"

**Testing:**
These ARE the tests.

**Verification:**
Run: `cargo test code_search`
Expected: All tests pass

**Commit:** `test(tools): integration tests for CodeSearchTool`
<!-- END_TASK_3 -->

<!-- END_SUBCOMPONENT_A -->
