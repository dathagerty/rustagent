# Rustagent V2 Phase 4: Test Requirements

**Generated:** 2026-02-11

**Source:** Acceptance criteria from `phase_01.md` through `phase_08.md`

**Test runner:** vitest (configured in phase_01.md, scripts: `bun run test`, `bun run test:watch`)

**Planned test files:**
- `web/src/router.test.ts` (phase_03.md Task 6)
- `web/src/lib/tree.test.ts` (phase_05.md Task 3)
- `web/src/lib/decision-graph.test.ts` (phase_06.md Task 4)

**Build verification:** `bun run build` and `npx tsc --noEmit` used throughout as structural checks

---

## 1. Summary Table

| AC ID | Description | Method | Detail |
|-------|-------------|--------|--------|
| P4a.AC1.1 | `web/` directory with config files | Human | Verify files exist after scaffolding |
| P4a.AC1.2 | `web/src/` contains main.ts and App.svelte | Human | Verify files exist after scaffolding |
| P4a.AC1.3 | `bun install` succeeds | Human | Run `bun install` in `web/`, check exit 0 |
| P4a.AC2.1 | `bun run dev` starts Vite on port 5173 | Human | Start dev server, verify port |
| P4a.AC2.2 | Browser shows placeholder content | Human | Open localhost:5173 in browser |
| P4a.AC3.1 | Vite proxies `/api/*` to daemon | Human | Start daemon + dev server, curl /api/health |
| P4a.AC3.2 | Vite proxies `/ws` WebSocket to daemon | Human | Start daemon + dev server, verify WS connection |
| P4b.AC1.1 | types.ts defines all API response interfaces | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC1.2 | WsEvent discriminated union covers 9 event types | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC1.3 | Enum literals match Rust serde serialization | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC2.1 | ApiClient typed methods for all 21+ endpoints | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC2.2 | Error responses throw typed ApiError | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC2.3 | ApiClient configurable base URL | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC3.1 | WsConnection dispatches typed events | Automated | `npx tsc --noEmit` (build check) |
| P4b.AC3.2 | Auto-reconnect with exponential backoff | Human | Observe reconnection by killing/restarting daemon |
| P4b.AC3.3 | Connection state exposed as reactive boolean | Automated | `npx tsc --noEmit` (build check) |
| P4c.AC1.1 | projects.svelte.ts manages project state | Automated | `npx tsc --noEmit` (build check) |
| P4c.AC1.2 | graph.svelte.ts manages graph state | Automated | `npx tsc --noEmit` (build check) |
| P4c.AC1.3 | agents.svelte.ts manages agent state | Automated | `npx tsc --noEmit` (build check) |
| P4c.AC1.4 | search.svelte.ts manages search state | Automated | `npx tsc --noEmit` (build check) |
| P4c.AC1.5 | Stores integrate with WebSocket events | Human | Create nodes via API while UI is open, verify updates |
| P4c.AC2.1 | App.svelte renders sidebar + main content | Human | Open dev server, verify layout |
| P4c.AC2.2 | Hash routing maps fragments to views | Automated | `web/src/router.test.ts` |
| P4c.AC2.3 | Navigation links update hash and render view | Human | Click sidebar links, verify URL and content |
| P4c.AC2.4 | Browser back/forward works with hash routing | Human | Navigate, use back/forward, verify |
| P4d.AC1.1 | Dashboard loads projects with goal counts | Human | Open `/#/`, verify project cards |
| P4d.AC1.2 | Dashboard shows active goals with status badges | Human | Open `/#/`, verify goal list |
| P4d.AC1.3 | Dashboard shows running agents summary | Human | Run agents, check dashboard display |
| P4d.AC2.1 | ProjectList shows projects with name, path, date | Human | Open `/#/projects`, verify table |
| P4d.AC2.2 | Clicking project navigates to ProjectDetail | Human | Click a project row, verify navigation |
| P4d.AC3.1 | ProjectDetail loads goals for selected project | Human | Open `/#/projects/<id>`, verify goals |
| P4d.AC3.2 | Goals show title, status, priority, completion % | Human | Inspect goal cards in ProjectDetail |
| P4d.AC3.3 | Navigation links to TaskTree, DecisionGraph, etc. | Human | Click goal action links, verify routes |
| P4e.AC1.1 | TaskTree renders hierarchical tree from Contains edges | Automated | `web/src/lib/tree.test.ts` |
| P4e.AC1.2 | Nodes show title, status, priority, assigned agent | Human | Open `/#/goals/<id>/tasks`, inspect nodes |
| P4e.AC1.3 | Tree nodes expand/collapse | Human | Click expand/collapse toggles |
| P4e.AC2.1 | Status badges use correct colors | Automated | `web/src/lib/tree.test.ts` (countTaskStats) |
| P4e.AC2.2 | Dependency edges shown as visual indicators | Human | Create depends-on edges, verify display |
| P4e.AC2.3 | Ready tasks visually highlighted | Human | Verify ready tasks have distinct styling |
| P4e.AC3.1 | Ready Tasks panel from /tasks/ready | Human | Verify ready tasks panel renders |
| P4e.AC3.2 | Next Task recommendation from /tasks/next | Human | Verify "next" task is highlighted |
| P4e.AC3.3 | Clicking node shows detail in side panel | Human | Click tree node, verify detail panel |
| P4f.AC1.1 | CytoscapeGraph initializes and destroys properly | Human | Navigate to/from decision graph, check no leaks |
| P4f.AC1.2 | Dagre layout renders top-to-bottom DAG | Human | Open decision graph, verify hierarchy |
| P4f.AC1.3 | Pan, zoom, drag interactions work | Human | Interact with graph canvas |
| P4f.AC2.1 | Decision nodes render as diamonds (gold) | Automated | `web/src/lib/decision-graph.test.ts` |
| P4f.AC2.2 | Option nodes render as hexagons (green/gray) | Automated | `web/src/lib/decision-graph.test.ts` |
| P4f.AC2.3 | Outcome and Revisit nodes have distinct shapes | Human | Visual inspection of graph |
| P4f.AC2.4 | Edge labels show relationship types | Human | Visual inspection of edge labels |
| P4f.AC3.1 | Now mode shows active/decided decisions only | Automated | `web/src/lib/decision-graph.test.ts` |
| P4f.AC3.2 | History mode shows full evolution | Automated | `web/src/lib/decision-graph.test.ts` |
| P4f.AC3.3 | Toggle between modes re-renders graph | Human | Click Now/History toggle, verify update |
| P4f.AC4.1 | Clicking node shows detail in side panel | Human | Click decision node, verify panel |
| P4f.AC4.2 | Option nodes show pros/cons from metadata | Human | Click option node, verify metadata display |
| P4f.AC4.3 | Fit to view button centers graph | Human | Click "Fit to view", verify centering |
| P4g.AC1.1 | AgentMonitor shows real-time WebSocket event feed | Human | Run agents, verify events appear |
| P4g.AC1.2 | Agent status badges show current state | Human | Verify badge colors during agent lifecycle |
| P4g.AC1.3 | Tool execution events show tool, args, result | Human | Verify tool events render with details |
| P4g.AC1.4 | Feed auto-scrolls and caps at 200 items | Human | Generate >200 events, verify cap and scroll |
| P4g.AC2.1 | SessionHistory shows sessions sorted newest-first | Human | Create sessions, verify order |
| P4g.AC2.2 | Sessions show timestamps, agents, summary | Human | Verify session card content |
| P4g.AC2.3 | Handoff notes displayed as formatted text | Human | Verify handoff notes rendering |
| P4g.AC3.1 | GraphSearch accepts text query and shows results | Human | Type query, verify results |
| P4g.AC3.2 | Results filterable by node type | Human | Toggle type filters, verify filtering |
| P4g.AC3.3 | Results show title, type badge, status, snippet | Human | Inspect result cards |
| P4g.AC3.4 | Clicking result navigates to appropriate view | Human | Click result, verify navigation |
| P4h.AC1.1 | `cargo build --features bundle-ui` succeeds | Human | Run command, verify exit 0 |
| P4h.AC1.2 | `cargo build` (no bundle-ui) succeeds | Human | Run command, verify exit 0 |
| P4h.AC1.3 | `web/dist/` contains index.html + bundles | Human | `ls web/dist/`, verify contents |
| P4h.AC2.1 | Daemon serves UI at localhost:7400 | Human | Start daemon, curl `/`, verify HTML |
| P4h.AC2.2 | SPA routing works in production | Human | Navigate to hash routes, verify |
| P4h.AC2.3 | Static assets served with correct Content-Type | Human | `curl -I` an asset, verify header |
| P4h.AC2.4 | API endpoints work alongside embedded UI | Human | `curl /api/health`, verify response |
| P4h.AC3.1 | Non-bundle-ui daemon shows helpful fallback message | Human | Start daemon without feature, curl `/`, verify message |

**Totals:** 68 acceptance criteria. 21 verified by automated tests or build checks. 47 require human verification.

---

## 2. Automated Test Requirements

### 2.1 Build Checks (`npx tsc --noEmit` and `bun run build`)

These acceptance criteria are verified structurally by the TypeScript compiler and Vite build. If the code compiles without errors, the interfaces, types, and function signatures exist and are correct. These are not vitest tests but are automated and reproducible.

**Verification command:** `cd web && npx tsc --noEmit && bun run build`

| AC ID | What the build check verifies |
|-------|-------------------------------|
| P4b.AC1.1 | `types.ts` exports all required interfaces; any consumer importing them would fail tsc if missing |
| P4b.AC1.2 | `WsEvent` discriminated union is used by websocket.ts event dispatch; tsc verifies exhaustive coverage |
| P4b.AC1.3 | Enum literal types are used in all type definitions; tsc ensures they are string literal unions |
| P4b.AC2.1 | `client.ts` exports typed async functions for all endpoints; tsc verifies return types match |
| P4b.AC2.2 | `ApiError` class is thrown in the request helper; tsc verifies it has `status` and `message` |
| P4b.AC2.3 | `baseUrl` parameter accepted by ApiClient constructor; tsc verifies type signature |
| P4b.AC3.1 | `WsConnection` class with `onEvent` callback typed to `WsEvent`; tsc verifies |
| P4b.AC3.3 | `connected` boolean field exists on WsConnection; tsc verifies type |
| P4c.AC1.1 | projects.svelte.ts exports `projectsState`, `selectedProject`, and action functions; tsc verifies |
| P4c.AC1.2 | graph.svelte.ts exports `graphState` and action functions with correct types; tsc verifies |
| P4c.AC1.3 | agents.svelte.ts exports `agentsState` and action functions; tsc verifies |
| P4c.AC1.4 | search.svelte.ts exports `searchState` and action functions; tsc verifies |

### 2.2 `web/src/router.test.ts`

**Test runner:** vitest
**Phase:** 4c (phase_03.md, Task 6)
**Function under test:** `matchRoute(path: string, routes: RouteDefinition[]): { name: string, params: Record<string, string> }`

| Test | AC ID | Description |
|------|-------|-------------|
| `matchRoute("/") returns dashboard` | P4c.AC2.2 | Root path matches the `dashboard` route with empty params |
| `matchRoute("/projects") returns project-list` | P4c.AC2.2 | Static path matches `project-list` with empty params |
| `matchRoute("/projects/ra-abcd") returns project-detail` | P4c.AC2.2 | Parameterized path extracts `{ projectId: "ra-abcd" }` |
| `matchRoute("/goals/ra-1234/tasks") returns task-tree` | P4c.AC2.2 | Parameterized path extracts `{ goalId: "ra-1234" }` |
| `matchRoute("/goals/ra-1234/decisions") returns decision-graph` | P4c.AC2.2 | Parameterized path extracts `{ goalId: "ra-1234" }` |
| `matchRoute("/goals/ra-1234/agents") returns agent-monitor` | P4c.AC2.2 | Parameterized path extracts `{ goalId: "ra-1234" }` |
| `matchRoute("/goals/ra-1234/sessions") returns session-history` | P4c.AC2.2 | Parameterized path extracts `{ goalId: "ra-1234" }` |
| `matchRoute("/search") returns graph-search` | P4c.AC2.2 | Static path matches `graph-search` with empty params |
| `matchRoute("/unknown/path") returns not-found` | P4c.AC2.2 | Unrecognized path falls through to `not-found` |
| `matchRoute("") returns dashboard` | P4c.AC2.2 | Empty string treated as root, returns `dashboard` |

### 2.3 `web/src/lib/tree.test.ts`

**Test runner:** vitest
**Phase:** 4e (phase_05.md, Task 3)
**Functions under test:** `buildTree`, `flattenTree`, `countTaskStats`

| Test | AC ID | Description |
|------|-------|-------------|
| `buildTree produces correct nesting from Contains edges` | P4e.AC1.1 | Root is the goal node, children are tasks connected by `Contains` edges, subtask nested under parent task |
| `buildTree sorts children by ID` | P4e.AC1.1 | Children at each level are sorted by dotted-hierarchy ID (e.g., `ra-xxxx.1` before `ra-xxxx.2`) |
| `buildTree identifies dependencies from dependson edges` | P4e.AC1.1 | Nodes with `dependson` edges have their `dependencies` array populated with the correct target nodes |
| `flattenTree returns depth-first order` | P4e.AC1.1 | Flattened output visits root, then first child and its subtree, then second child, etc. |
| `countTaskStats returns correct status counts` | P4e.AC2.1 | Given fixture with 1 completed, 1 in_progress, and 1 ready task, returns `{ total: 3, completed: 1, inProgress: 1, blocked: 0, ready: 1 }` |
| `buildTree handles empty input` | P4e.AC1.1 | Empty nodes/edges array produces a sensible result or throws a descriptive error |

### 2.4 `web/src/lib/decision-graph.test.ts`

**Test runner:** vitest
**Phase:** 4f (phase_06.md, Task 4)
**Functions under test:** `decisionsToElements`, `filterNowMode`, `decisionStylesheet`

| Test | AC ID | Description |
|------|-------|-------------|
| `decisionsToElements converts nodes with correct data.type` | P4f.AC2.1, P4f.AC2.2 | Decision nodes get `data.type = "decision"`, option nodes get `data.type = "option"`, etc.; these types drive Cytoscape stylesheet selectors for shapes (diamond, hexagon) |
| `decisionsToElements converts nodes with correct data.status` | P4f.AC2.1, P4f.AC2.2 | Status field preserved for chosen/rejected styling in stylesheet |
| `decisionsToElements converts edges with source, target, edgeType` | P4f.AC2.1, P4f.AC2.2 | Edges have `data.source = from_node`, `data.target = to_node`, `data.edgeType` matching edge_type |
| `filterNowMode includes active/decided decisions and chosen options` | P4f.AC3.1 | Active decisions, decided decisions, and chosen options pass the filter |
| `filterNowMode excludes rejected/abandoned/superseded nodes` | P4f.AC3.1 | Nodes with these statuses are filtered out |
| `filterNowMode excludes edges with filtered-out endpoints` | P4f.AC3.1 | If either endpoint of an edge was removed, the edge is also removed |
| `without filterNowMode all nodes are present (History mode)` | P4f.AC3.2 | `decisionsToElements` called on unfiltered data includes all node types and statuses |
| `decisionsToElements handles empty input` | P4f.AC2.1 | Empty arrays produce empty output |
| `decisionStylesheet returns non-empty array` | P4f.AC2.1 | Stylesheet array has entries for decision, option, outcome, revisit, and edge styling |

---

## 3. Human Verification Requirements

### 3.1 Phase 4a: Project Scaffolding

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4a.AC1.1 | After scaffolding, verify `web/` contains `package.json`, `vite.config.ts`, `svelte.config.js`, `tsconfig.json`, `index.html` | File existence check after creation; could be scripted but is a one-time setup task |
| P4a.AC1.2 | Verify `web/src/main.ts` and `web/src/App.svelte` exist | File existence check after creation |
| P4a.AC1.3 | Run `cd web && bun install`, verify exit code 0 and `node_modules/` created | Package manager operation; depends on network and Bun runtime |
| P4a.AC2.1 | Run `cd web && bun run dev`, verify Vite outputs "Local: http://localhost:5173" | Dev server startup is a runtime behavior that depends on port availability |
| P4a.AC2.2 | Open `http://localhost:5173` in browser, verify "Rustagent Web UI" text renders | Visual rendering in browser cannot be unit tested without a headless browser framework |
| P4a.AC3.1 | Start daemon and dev server, `curl http://localhost:5173/api/health` returns `{"status":"ok"}` | Proxy integration requires two running processes (daemon + Vite) |
| P4a.AC3.2 | Start daemon and dev server, verify WebSocket connection at `ws://localhost:5173/ws` | WebSocket proxy requires both daemon and Vite running |

### 3.2 Phase 4b: API Client + WebSocket

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4b.AC3.2 | Kill daemon while UI is open, observe reconnection attempts in console with backoff (1s, 2s, 4s, 8s), then restart daemon and verify reconnection | Reconnection behavior requires a running WebSocket server and network interruption simulation |

### 3.3 Phase 4c: Stores + App Shell + Routing

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4c.AC1.5 | Open UI in browser, create a node via `curl` to the daemon API, verify the UI updates without manual refresh | WebSocket-to-store integration requires running daemon and browser |
| P4c.AC2.1 | Open dev server in browser, verify sidebar (left, ~240px) and main content area render | Visual layout verification |
| P4c.AC2.3 | Click each sidebar link ("Dashboard", "Projects", "Search"), verify URL hash changes and correct placeholder/view renders | Interactive browser behavior |
| P4c.AC2.4 | Navigate through several views, press browser Back and Forward buttons, verify correct view renders each time | Browser history integration |

### 3.4 Phase 4d: Dashboard + Project Views

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4d.AC1.1 | Start daemon with test data, open `/#/`, verify project cards with goal counts | Requires running daemon with populated data |
| P4d.AC1.2 | Verify active goals section lists goals with colored status badges | Visual styling verification |
| P4d.AC1.3 | Run agents against a goal, verify dashboard shows agent count and task info | Requires live agent execution |
| P4d.AC2.1 | Open `/#/projects`, verify table shows project name, path (monospace), and formatted date | Visual layout and data formatting |
| P4d.AC2.2 | Click a project in the list, verify URL changes to `/#/projects/<id>` and ProjectDetail renders | Interactive navigation |
| P4d.AC3.1 | Open `/#/projects/<id>`, verify goals load for the correct project | Data-driven view rendering |
| P4d.AC3.2 | Inspect goal cards: title, status badge (colored), priority badge, and completion percentage (X/Y tasks) | Visual display of computed data |
| P4d.AC3.3 | Click "Tasks", "Decisions", "Agents", "Sessions" links on a goal card, verify navigation to `/#/goals/<goalId>/tasks`, etc. | Interactive navigation |

### 3.5 Phase 4e: Task Tree View

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4e.AC1.2 | Open task tree, verify each node row shows title, status badge, priority badge, and agent chip | Visual component composition |
| P4e.AC1.3 | Click expand/collapse toggle on a node with children, verify children show/hide | Interactive DOM manipulation |
| P4e.AC2.2 | Create a task with a `dependson` edge, verify the tree shows a "depends on: ra-xxxx.N" indicator | Visual dependency rendering |
| P4e.AC2.3 | Set a task to "ready" status, verify it has a distinct background/border color in the tree | Visual styling |
| P4e.AC3.1 | Open task tree, verify the bottom panel lists tasks returned by `/tasks/ready` | Data-driven panel rendering |
| P4e.AC3.2 | Open task tree, verify the "Next recommended" task is highlighted in the ready tasks panel | Visual highlight driven by API data |
| P4e.AC3.3 | Click a tree node, verify the right panel shows full description, acceptance criteria, dependencies, and edges | Interactive detail panel |

### 3.6 Phase 4f: Decision Graph View

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4f.AC1.1 | Navigate to decision graph and away, check browser console for no Cytoscape errors or memory leaks | Cytoscape lifecycle requires DOM interaction |
| P4f.AC1.2 | Open decision graph with test data, verify nodes arrange top-to-bottom via dagre | Graph layout is visual |
| P4f.AC1.3 | Pan (click-drag background), zoom (scroll wheel), and drag (click-drag node) in the graph | Interactive canvas behavior |
| P4f.AC2.3 | Verify outcome nodes render as ellipses and revisit nodes as triangles with distinct colors | Visual shape verification |
| P4f.AC2.4 | Verify edges display labels like "chosen", "rejected", "leads_to" | Visual label rendering |
| P4f.AC3.3 | Click "Now" and "History" toggle buttons, verify graph re-renders with different node sets | Interactive mode switching |
| P4f.AC4.1 | Click a node in the graph, verify detail panel appears with GraphNodeCard | Interactive panel |
| P4f.AC4.2 | Click an option node that has `pros`/`cons` in metadata, verify they display as styled lists | Metadata rendering |
| P4f.AC4.3 | Click "Fit to view" button, verify graph centers and zooms to show all nodes | Interactive graph control |

### 3.7 Phase 4g: Agent Monitor + Session History + Graph Search

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4g.AC1.1 | Run an agent, open `/#/goals/<id>/agents`, verify events appear in the feed as they happen | Real-time WebSocket event rendering |
| P4g.AC1.2 | During agent execution, verify badge colors change (blue for spawning, green for working, etc.) | Visual status transitions |
| P4g.AC1.3 | During agent execution, verify tool events show tool name, truncated args, and result summary | Visual content rendering |
| P4g.AC1.4 | Generate many events (or manually add), verify feed stops at 200 items and auto-scrolls to newest | Feed behavior at scale |
| P4g.AC2.1 | Create multiple sessions for a goal, open `/#/goals/<id>/sessions`, verify newest-first ordering | Data ordering |
| P4g.AC2.2 | Verify each session card shows start/end time, duration, agent IDs as chips, and summary text | Visual card composition |
| P4g.AC2.3 | Create a session with handoff notes containing `## Done`, `## Remaining` sections, verify formatted rendering | Preformatted text rendering with headers |
| P4g.AC3.1 | Open `/#/search`, type a query, click search, verify matching nodes appear | Search integration requires running daemon with indexed data |
| P4g.AC3.2 | After searching, click type filter buttons ("Goals", "Tasks", etc.), verify results update | Interactive filter behavior |
| P4g.AC3.3 | Inspect result cards for title, type badge, status badge, and description snippet (truncated to ~150 chars) | Visual card composition |
| P4g.AC3.4 | Click a goal result, verify navigation to `/#/goals/<id>/tasks`; click a decision result, verify navigation to `/#/goals/<id>/decisions` | Navigation routing from search results |

### 3.8 Phase 4h: Build Integration + Production Serving

| AC ID | Verification Approach | Justification |
|-------|----------------------|---------------|
| P4h.AC1.1 | Run `cargo build --features bundle-ui`, verify exit code 0 and build log shows web build output | Cargo build with feature flag; requires Bun installed |
| P4h.AC1.2 | Run `cargo build` (without feature), verify exit code 0 and build log does NOT show web build | Cargo build without feature flag |
| P4h.AC1.3 | After `bun run build`, run `ls web/dist/`, verify `index.html` and `assets/` directory with JS/CSS files | Build output verification |
| P4h.AC2.1 | Build with `bundle-ui`, start daemon, `curl http://localhost:7400/` returns HTML containing "Rustagent" | Production serving integration |
| P4h.AC2.2 | In browser, navigate to `http://localhost:7400/#/projects`, verify the SPA route loads correctly | Hash-based SPA routing in production |
| P4h.AC2.3 | `curl -I http://localhost:7400/assets/<hash>.js` returns `Content-Type: application/javascript` | MIME type detection in rust-embed serving |
| P4h.AC2.4 | While serving UI, `curl http://localhost:7400/api/health` returns `{"status":"ok"}` | API and UI coexistence |
| P4h.AC3.1 | Build without `bundle-ui`, start daemon, `curl http://localhost:7400/` returns message with "bun run dev" and "--features bundle-ui" instructions | Graceful fallback message |

---

## 4. Architecture-Level Verification

The v2-architecture.md specifies one overarching verification for Phase 4:

> **Verification:** `bun run dev` in `web/` shows dashboard. Creating a goal via API updates the UI in real-time via WebSocket.

This maps to:
- **P4a.AC2.1 + P4a.AC2.2**: Dev server starts and renders the UI
- **P4d.AC1.1 + P4d.AC1.2**: Dashboard shows projects and goals
- **P4c.AC1.5**: WebSocket events update stores
- **P4h Task 4**: End-to-end production WebSocket verification

This is a **human verification** task (Phase 4h, Task 4) that exercises the full stack: daemon API, WebSocket broadcast, Svelte store integration, and reactive UI update. Procedure:

1. Build and start daemon with `bundle-ui`
2. Open `http://localhost:7400` in browser
3. Create a project via `curl -X POST http://localhost:7400/api/projects ...`
4. Create a goal via `curl -X POST http://localhost:7400/api/projects/<id>/goals ...`
5. Verify the Dashboard updates without page refresh

---

## 5. Coverage Analysis

### What is automated
- **Route pattern matching**: 10 test cases covering all 8 routes plus edge cases (router.test.ts)
- **Tree transformation**: 6 test cases covering hierarchy building, sorting, dependencies, flattening, and status counting (tree.test.ts)
- **Decision graph transformation**: 9 test cases covering element conversion, Now/History filtering, and stylesheet existence (decision-graph.test.ts)
- **Type correctness**: All TypeScript types, interfaces, store shapes, and API client signatures verified by `tsc --noEmit`
- **Build integrity**: `bun run build` verifies the full Svelte/Vite compilation pipeline

### What is not automated (and why)
- **Visual appearance** (colors, shapes, layout, badges): Requires a visual regression framework (e.g., Storybook + Chromatic) not in scope for this phase
- **Interactive behavior** (click, expand/collapse, pan/zoom, back/forward): Requires a browser automation framework (e.g., Playwright) running against the full app
- **WebSocket integration** (real-time updates, reconnection): Requires a running daemon and WebSocket server
- **Build pipeline** (cargo build, bun install, daemon start): Requires specific system tooling (Rust, Bun) and cannot run in a pure JS test environment
- **Production serving** (embedded assets, SPA fallback, MIME types): Requires the compiled Rust binary with `bundle-ui` feature

### Recommendations for future automation
1. **Playwright e2e tests**: Add a `web/e2e/` directory with Playwright tests for interactive flows (navigation, expand/collapse, search). These would cover P4c.AC2.3, P4c.AC2.4, P4d.AC2.2, P4e.AC1.3, P4g.AC3.2, etc.
2. **Component tests with @testing-library/svelte**: Test individual Svelte components (StatusBadge, GraphNodeCard, SearchResult) with vitest + jsdom. Would cover P4e.AC1.2, P4g.AC3.3, etc.
3. **CI build verification**: A CI step that runs `cd web && bun install && bun run build && bun run test` to catch regressions. Would cover P4h.AC1.3 automatically.
