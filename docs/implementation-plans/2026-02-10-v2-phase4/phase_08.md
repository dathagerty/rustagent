# Rustagent V2 Phase 4h: Build Integration + Production Serving

**Goal:** Verify the full build pipeline works end-to-end: `cargo build --features bundle-ui` compiles the Svelte app via build.rs and embeds it in the binary. The daemon serves the embedded UI at `/*` with SPA fallback routing.

**Architecture:** The build.rs (already implemented in Phase 3) runs `bun install && bun run build` in `web/` when the `bundle-ui` feature is enabled. rust-embed compiles `web/dist/` into the binary. The daemon's static_files.rs (already implemented) serves embedded assets with MIME type detection and SPA fallback (index.html for unknown paths). This phase verifies everything works together and adds the `.gitignore` for build artifacts.

**Tech Stack:** Rust (build.rs, rust-embed, mime_guess), Bun, Vite

**Scope:** Phase 8 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4h.AC1: Build pipeline
- **P4h.AC1.1 Success:** `cargo build --features bundle-ui` succeeds — build.rs runs bun install and bun run build, then the Rust compilation includes embedded assets
- **P4h.AC1.2 Success:** `cargo build` (without bundle-ui) succeeds — build.rs skips frontend build, no embedded assets
- **P4h.AC1.3 Success:** `web/dist/` is produced by `bun run build` and contains index.html + JS/CSS bundles

### P4h.AC2: Production serving
- **P4h.AC2.1 Success:** Daemon started from a `bundle-ui` build serves the UI at `http://localhost:7400/`
- **P4h.AC2.2 Success:** SPA routing works — `http://localhost:7400/#/projects` loads correctly (all hash routes work since they hit the same index.html)
- **P4h.AC2.3 Success:** Static assets (JS, CSS) are served with correct Content-Type headers
- **P4h.AC2.4 Success:** API endpoints still work alongside the embedded UI (`/api/*` routes take priority over UI fallback)

### P4h.AC3: Development mode fallback
- **P4h.AC3.1 Success:** Daemon started WITHOUT bundle-ui returns a helpful message at `/` directing users to run `bun run dev` in `web/` or build with `--features bundle-ui`

---

<!-- START_TASK_1 -->
### Task 1: Add web/ .gitignore and verify Vite build output

**Files:**
- Create: `web/.gitignore`

**Implementation:**

Create `web/.gitignore` to exclude build artifacts and dependencies:

```
node_modules/
dist/
.vite/
*.tsbuildinfo
```

Note: The bun lockfile (`bun.lockb` or `bun.lock`) is intentionally NOT in .gitignore — lockfiles should be committed for reproducible builds.

Then verify the Vite build produces correct output:

Run: `cd web && bun install && bun run build`
Expected: `web/dist/` is created containing:
- `index.html` (entry point with script tags pointing to JS bundle)
- `assets/` directory with hashed JS and CSS bundles

**Verification:**

Run: `cd web && bun run build && ls -la dist/`
Expected: dist/ contains index.html and assets/ with JS/CSS files.

**Commit:** `chore(web): add .gitignore for build artifacts`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Verify bundle-ui cargo build

**Files:**
- None (verification only)

**Implementation:**

Verify the full Rust build with embedded UI works:

1. Ensure `web/` has been built (Task 1 should have done this).
2. Run `cargo build --features bundle-ui`. This triggers build.rs which:
   - Checks `web/` exists
   - Runs `bun install` in `web/`
   - Runs `bun run build` in `web/`
   - Then rust-embed compiles `web/dist/` into the binary.

3. Also verify `cargo build` (without the feature) still works — build.rs should skip the web build entirely.

**Verification:**

Run: `cargo build --features bundle-ui`
Expected: Compilation succeeds with no errors. Build log shows web build output.

Run: `cargo build`
Expected: Compilation succeeds without attempting web build.

No commit for this task (verification only).

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Verify production serving end-to-end

**Files:**
- None (verification only)

**Implementation:**

Test the full production serving flow:

1. Build with bundle-ui: `cargo build --features bundle-ui`
2. Start the daemon: `cargo run --features bundle-ui -- daemon start`
3. Verify:
   - `curl http://localhost:7400/` returns HTML (the embedded index.html)
   - `curl http://localhost:7400/api/health` returns `{"status":"ok"}` (API still works)
   - `curl -I http://localhost:7400/assets/<hash>.js` returns `Content-Type: application/javascript` (correct MIME type)
   - Open `http://localhost:7400` in browser — the Svelte UI loads and renders
   - Navigate to `http://localhost:7400/#/projects` — SPA routing works (all hash routes work since they load the same index.html)

4. Stop the daemon.

5. Test the fallback mode:
   - `cargo build` (without bundle-ui)
   - `cargo run -- daemon start`
   - `curl http://localhost:7400/` returns the "UI not bundled" message directing to Vite dev server or `--features bundle-ui`
   - Stop the daemon.

**Verification:**

This is a human verification task. Run the steps above manually and confirm each assertion.

No commit for this task (verification only).

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Verify WebSocket real-time updates in production mode

**Files:**
- None (verification only)

**Implementation:**

This verifies the architecture doc's Phase 4 acceptance criterion: "Creating a goal via API updates the UI in real-time via WebSocket."

1. Build and start with bundle-ui: `cargo build --features bundle-ui && cargo run --features bundle-ui -- daemon start`
2. Open `http://localhost:7400` in browser — UI should load.
3. Register a test project: `curl -X POST http://localhost:7400/api/projects -H 'Content-Type: application/json' -d '{"name":"test","path":"/tmp/test"}'`
4. Navigate to the Dashboard in the UI.
5. Create a goal via API: `curl -X POST 'http://localhost:7400/api/projects/<project-id>/goals' -H 'Content-Type: application/json' -d '{"title":"Test Goal","description":"Testing real-time updates","priority":"medium"}'`
6. Verify: The Dashboard or ProjectDetail view updates to show the new goal without a manual page refresh (via WebSocket `node_created` event).

**Verification:**

This is a human verification task. The WebSocket event delivery depends on the bridge in ws.rs being properly connected to the message bus.

No commit for this task (verification only).

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Final cleanup and documentation commit

**Files:**
- Modify: `web/package.json` (if needed — ensure scripts are correct)

**Implementation:**

Review all web/ files created across Phases 1-8:
1. Verify `bun run build` produces clean output with no warnings.
2. Verify `bun run dev` starts without errors.
3. Verify all views render (even if some show placeholder data when no daemon data exists).
4. Verify all navigation links work.
5. Remove any `<Placeholder>` components if all views are implemented (they should all be replaced by now).

Clean up any unused imports or dead code.

**Verification:**

Run: `cd web && bun run build`
Expected: Clean build with no warnings.

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `chore(web): cleanup and verify all views build cleanly`

<!-- END_TASK_5 -->
