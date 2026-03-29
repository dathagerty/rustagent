# V2 Phase 5 - AGENTS.md Parser Enhancement

**Goal:** Enhance the AGENTS.md parser to count content lines per heading section and make `resolve_agents_md` return relative paths from the project root.

**Architecture:** The existing `src/context/agents_md.rs` already handles hierarchy walking and heading extraction. This phase adds per-heading content line counting (for `{rule_count} rules` in context summaries) and switches path output from absolute to project-relative. The `ReadAgentsMdTool` in `src/context/mod.rs` is enhanced to accept a directory path and auto-locate the AGENTS.md within it.

**Tech Stack:** Rust (standard library Path operations, no new dependencies)

**Scope:** 1 of 6 phases from original design (Phase 5, item 1)

**Codebase verified:** 2026-02-12

---

## Acceptance Criteria Coverage

This phase implements and tests:

### v2-phase5.AC1: AGENTS.md Resolution
- **v2-phase5.AC1.1 Success:** `resolve_agents_md` returns headings with line counts for each AGENTS.md in the hierarchy, ordered closest-to-file first
- **v2-phase5.AC1.2 Success:** Returned paths are relative to the project root (e.g., `src/auth/AGENTS.md`), not absolute
- **v2-phase5.AC1.3 Success:** Deduplication still works — same AGENTS.md is not returned twice when multiple files in scope share it

### v2-phase5.AC2: ReadAgentsMdTool Enhancement
- **v2-phase5.AC2.1 Success:** `read_agents_md` tool accepts a directory path (e.g., `src/auth`) and reads the AGENTS.md within that directory
- **v2-phase5.AC2.2 Success:** `read_agents_md` tool still accepts a full file path (e.g., `src/auth/AGENTS.md`) for backward compatibility

---

<!-- START_SUBCOMPONENT_A (tasks 1-3) -->

<!-- START_TASK_1 -->
### Task 1: Enhance heading extraction with content line counts

**Verifies:** v2-phase5.AC1.1

**Files:**
- Modify: `src/context/agents_md.rs:60-72` (replace `extract_headings` with `extract_heading_summaries`)

**Implementation:**

Replace the `extract_headings` function with `extract_heading_summaries` that returns `Vec<(String, usize)>` — each entry is `(heading_text, line_count)` where `line_count` is the number of non-empty content lines under that heading (until the next heading of same or higher level, or EOF).

Algorithm:
1. Split file content into lines
2. Walk lines: detect headings as lines starting with `# ` (single `#` followed by a space — top-level headings only, matching the existing `extract_headings` behavior at `agents_md.rs:63`). Do NOT match `##` or deeper headings.
3. For each heading, count subsequent non-empty, non-heading lines until the next `# ` line or EOF
4. Return `Vec<(String, usize)>`

Update `resolve_agents_md` to use `extract_heading_summaries` and format the summary as `"Heading1 (N lines), Heading2 (M lines)"`.

**Testing:**
Tests must verify:
- v2-phase5.AC1.1: Multiple headings return correct line counts; headings with no content return 0

**Verification:**
Run: `cargo test agents_md`
Expected: All tests pass

**Commit:** `feat(context): add content line counts to AGENTS.md heading extraction`
<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Return relative paths from resolve_agents_md

**Verifies:** v2-phase5.AC1.2, v2-phase5.AC1.3

**Files:**
- Modify: `src/context/agents_md.rs:14-57` (update path construction in `resolve_agents_md`)

**Implementation:**

In `resolve_agents_md`, after finding an AGENTS.md file, strip the `project_root` prefix from the absolute path to produce a relative path string (e.g., `src/auth/AGENTS.md` instead of `/home/user/project/src/auth/AGENTS.md`). Use `Path::strip_prefix(project_root)` and fall back to the absolute path if stripping fails.

**Testing:**
Tests must verify:
- v2-phase5.AC1.2: Returned paths are relative (don't start with `/tmp/` or whatever tempdir prefix the test uses)
- v2-phase5.AC1.3: Deduplication still works with relative paths

**Verification:**
Run: `cargo test agents_md`
Expected: All tests pass

**Commit:** `feat(context): return relative paths from resolve_agents_md`
<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Enhance ReadAgentsMdTool to accept directory paths

**Verifies:** v2-phase5.AC2.1, v2-phase5.AC2.2

**Files:**
- Modify: `src/context/mod.rs:130-149` (update `ReadAgentsMdTool::execute`)

**Implementation:**

Update the `execute` method to:
1. Check if the path ends with `AGENTS.md` — if so, read it directly (backward compat, AC2.2)
2. Otherwise, treat the path as a directory and append `/AGENTS.md` to it before reading
3. Keep the existing file-name validation for the direct path case

Update the tool description to mention it accepts either a directory path or a direct AGENTS.md path.

**Testing:**
Tests must verify:
- v2-phase5.AC2.1: Passing `src/auth` reads `src/auth/AGENTS.md`
- v2-phase5.AC2.2: Passing `src/auth/AGENTS.md` still works

**Verification:**
Run: `cargo test context`
Expected: All tests pass

**Commit:** `feat(context): ReadAgentsMdTool accepts directory paths`
<!-- END_TASK_3 -->

<!-- END_SUBCOMPONENT_A -->
