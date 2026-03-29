# Rustagent V2 Phase 4a: Web UI Project Scaffolding

**Goal:** Initialize the `web/` directory with Bun + Vite + Svelte 5 + TypeScript, configure the Vite dev server proxy for the daemon API, and verify the scaffold builds and runs.

**Architecture:** The web UI is a Svelte 5 SPA served by the daemon (embedded via rust-embed in production, or via Vite dev server during development). It lives in `web/` at the repo root. Bun is the package manager (build toolchain only — runtime is the browser). No SSR.

**Tech Stack:** Svelte 5, Vite 6+, TypeScript 5, Bun

**Scope:** Phase 1 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4a.AC1: Project structure
- **P4a.AC1.1 Success:** `web/` directory exists with package.json, vite.config.ts, svelte.config.js, tsconfig.json, index.html
- **P4a.AC1.2 Success:** `web/src/` contains main.ts and App.svelte entry points
- **P4a.AC1.3 Success:** `bun install` in `web/` succeeds without errors

### P4a.AC2: Dev server
- **P4a.AC2.1 Success:** `bun run dev` in `web/` starts the Vite dev server (default port 5173)
- **P4a.AC2.2 Success:** Browser at `http://localhost:5173` shows the placeholder App.svelte content

### P4a.AC3: API proxy
- **P4a.AC3.1 Success:** Vite dev server proxies `/api/*` requests to `http://localhost:7400/api/*`
- **P4a.AC3.2 Success:** Vite dev server proxies `/ws` WebSocket connections to `ws://localhost:7400/ws`

---

<!-- START_TASK_1 -->
### Task 1: Create web/ directory structure and package.json

**Files:**
- Create: `web/package.json`

**Implementation:**

Create `web/package.json` with the following content. Use `"type": "module"` for ESM. Scripts: `dev` (vite), `build` (vite build), `preview` (vite preview), `test` (vitest run), `test:watch` (vitest).

Dependencies: none (runtime dependencies added in later phases as needed).

Dev dependencies: `svelte`, `@sveltejs/vite-plugin-svelte`, `vite`, `typescript`, `@tsconfig/svelte`, `vitest`.

Use caret ranges (`^`) for all versions. The architecture doc specifies svelte ^5, vite ^6, but use current compatible versions. Do not pin exact versions.

**Verification:**

The file is valid JSON.

**Commit:** `chore(web): initialize package.json with Svelte 5 + Vite dependencies`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create Vite and Svelte config files

**Files:**
- Create: `web/vite.config.ts`
- Create: `web/svelte.config.js`
- Create: `web/tsconfig.json`

**Implementation:**

**vite.config.ts**: Import `defineConfig` from `vite` and `svelte` from `@sveltejs/vite-plugin-svelte`. Configure the `svelte()` plugin. Add a `server.proxy` block:
- `/api` → target `http://localhost:7400`, `changeOrigin: true` (no rewrite — daemon expects `/api` prefix)
- `/ws` → target `ws://localhost:7400`, `ws: true`, `changeOrigin: true`

Set `build.outDir` to `dist` (Vite default, matches rust-embed `#[folder = "web/dist/"]`).

**svelte.config.js**: Import `vitePreprocess` from `@sveltejs/vite-plugin-svelte`. Export a config with `preprocess: [vitePreprocess()]`.

**tsconfig.json**: Extend `@tsconfig/svelte`. Set `compilerOptions`: `target: "ESNext"`, `module: "ESNext"`, `moduleResolution: "bundler"`, `resolveJsonModule: true`, `isolatedModules: true`, `strict: true`, `verbatimModuleSyntax: true`. Include `["src/**/*"]`.

**Verification:**

Files are syntactically correct TypeScript/JavaScript/JSON.

**Commit:** `chore(web): add Vite, Svelte, and TypeScript config files`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Create index.html and Svelte entry points

**Files:**
- Create: `web/index.html`
- Create: `web/src/main.ts`
- Create: `web/src/App.svelte`

**Implementation:**

**index.html**: Standard HTML5 boilerplate. Charset UTF-8, viewport meta tag. A `<div id="app"></div>` mount point. A `<script type="module" src="/src/main.ts"></script>` entry. Title: "Rustagent".

**src/main.ts**: Import `App` from `./App.svelte`. Import a global CSS file `./styles/global.css` (will be created). Mount the app: `import { mount } from 'svelte'; const app = mount(App, { target: document.getElementById('app')! }); export default app;`

Note: Svelte 5 uses `mount()` from `'svelte'` instead of `new App()`. Check the Svelte 5 docs — if the version uses the constructor pattern, use `new App({ target: ... })` instead.

**src/App.svelte**: A minimal placeholder component. Use `<script lang="ts">` with a `let message = $state('Rustagent Web UI')` rune. Render `<main><h1>{message}</h1><p>Web UI is running.</p></main>`.

**src/styles/global.css**: Minimal reset — `box-sizing: border-box` on all elements, `margin: 0` on body, `font-family: system-ui, sans-serif` on body. Dark theme defaults: `background: #1a1a2e`, `color: #e0e0e0`.

**Verification:**

Files exist in correct locations.

**Commit:** `chore(web): add index.html and Svelte entry points`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Install dependencies and verify dev server

**Files:**
- None (operational verification)

**Implementation:**

Run `bun install` in the `web/` directory. Verify it creates `bun.lockb` (or `bun.lock`) and `node_modules/`.

Then run `bun run dev` and verify the Vite dev server starts on port 5173 (or the first available port). Open `http://localhost:5173` in a browser (or curl) and verify it returns HTML containing the App.svelte placeholder content.

Stop the dev server after verification.

**Verification:**

Run: `cd web && bun install`
Expected: Exits 0, node_modules/ created.

Run: `cd web && bun run build`
Expected: Exits 0, web/dist/ created with index.html and JS bundle.

**Commit:** `chore(web): verify scaffold builds successfully`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Verify Vite proxy configuration

**Files:**
- None (operational verification)

**Implementation:**

This task verifies the proxy works end-to-end. Start the daemon (`cargo run -- daemon start`) and the Vite dev server (`cd web && bun run dev`).

1. Open `http://localhost:5173/api/health` in the browser — should return `{"status":"ok"}` (proxied to daemon).
2. Open `http://localhost:5173` — should show the Svelte placeholder page.

If the daemon is not running, the proxy requests will fail with connection refused — this is expected and correct. The proxy configuration itself is verified by step 1 when the daemon is available.

**Verification:**

Run: `cd web && bun run dev` (in background)
Then: `curl http://localhost:5173` returns HTML with "Rustagent"
Expected: Dev server serves the SPA.

No commit for this task (verification only).

<!-- END_TASK_5 -->
