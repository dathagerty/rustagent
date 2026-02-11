# Rustagent V2 Phase 4c: Reactive Stores + App Shell + Routing

**Goal:** Create Svelte 5 runes-based reactive stores for all application state, build the app shell layout with navigation, and implement hash-based SPA routing.

**Architecture:** Stores use Svelte 5 `.svelte.ts` files with `$state` runes for shared reactive state. The app shell provides a sidebar navigation and main content area. Routing is hash-based (`/#/projects`, `/#/goals/:id/tasks`, etc.) — simple, no external router dependency, works with static file serving and SPA fallback.

**Tech Stack:** Svelte 5 (runes: $state, $derived, $effect), TypeScript

**Scope:** Phase 3 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4c.AC1: Reactive stores
- **P4c.AC1.1 Success:** `projects.svelte.ts` manages project list and selected project state
- **P4c.AC1.2 Success:** `graph.svelte.ts` manages nodes, edges, goal tree, tasks, and decisions state
- **P4c.AC1.3 Success:** `agents.svelte.ts` manages active agents list and WebSocket event feed
- **P4c.AC1.4 Success:** `search.svelte.ts` manages search query, results, and filters
- **P4c.AC1.5 Success:** Stores integrate with WebSocket — incoming events update relevant stores

### P4c.AC2: App shell and routing
- **P4c.AC2.1 Success:** App.svelte renders a layout with sidebar navigation and main content area
- **P4c.AC2.2 Success:** Hash-based routing maps URL fragments to view components
- **P4c.AC2.3 Success:** Navigation links update the hash and render the correct view
- **P4c.AC2.4 Success:** Browser back/forward buttons work with hash routing

---

<!-- START_SUBCOMPONENT_A (tasks 1-4) -->

<!-- START_TASK_1 -->
### Task 1: Create projects store

**Files:**
- Create: `web/src/stores/projects.svelte.ts`

**Implementation:**

Create a reactive store using Svelte 5 runes for project state. Export the following:

- `projectsState` — `$state` object with fields:
  - `projects: ProjectResponse[]` (list of all projects)
  - `selectedProjectId: string | null` (currently selected project)
  - `loading: boolean`
  - `error: string | null`

- `selectedProject` — `$derived` value: the project from `projects` matching `selectedProjectId`, or `null`.

- Action functions:
  - `async loadProjects()` — calls `listProjects()` from the API client, updates `projects`. Sets `loading`/`error` appropriately.
  - `selectProject(id: string)` — sets `selectedProjectId`.
  - `async createProject(name: string, path: string)` — calls `createProject()`, appends to `projects`.
  - `async removeProject(id: string)` — calls `deleteProject()`, removes from `projects`.

Import types and API functions from `../types` and `../api/client`.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add projects reactive store`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create graph store

**Files:**
- Create: `web/src/stores/graph.svelte.ts`

**Implementation:**

Create a reactive store for work graph state. This is the largest store — it manages nodes, edges, goal trees, tasks, and decisions.

- `graphState` — `$state` object:
  - `goalTree: GoalTree | null` (current goal's full node/edge tree)
  - `selectedGoalId: string | null`
  - `selectedNodeId: string | null`
  - `selectedNodeDetail: NodeWithEdges | null`
  - `tasks: GraphNode[]` (task nodes for current goal)
  - `readyTasks: GraphNode[]`
  - `decisions: GraphNode[]` (active decisions for current project)
  - `decisionHistory: DecisionHistory | null`
  - `loading: boolean`
  - `error: string | null`

- Derived values:
  - `goalNodes` — `$derived`: filter `goalTree?.nodes` to `node_type === 'goal'`
  - `tasksByStatus` — `$derived`: group `tasks` by `status` field into a `Record<NodeStatus, GraphNode[]>`

- Action functions:
  - `async loadGoals(projectId: string)` — calls `listGoals()`, stores as goals
  - `selectGoal(id: string)` — sets `selectedGoalId`
  - `async loadGoalTree(goalId: string)` — calls `getGoalTree()`, stores result
  - `async loadTasks(goalId: string)` — calls `listTasks()`, stores result
  - `async loadReadyTasks(goalId: string)` — calls `listReadyTasks()`
  - `async loadDecisions(projectId: string)` — calls `listDecisions()`
  - `async loadDecisionHistory(projectId: string)` — calls `getDecisionHistory()`
  - `async loadNodeDetail(nodeId: string)` — calls `getNode()`, stores in `selectedNodeDetail`
  - `selectNode(id: string)` — sets `selectedNodeId` and calls `loadNodeDetail`
  - `clearSelection()` — resets selected node

- WebSocket integration function:
  - `handleWsEvent(event: WsEvent)` — processes relevant events:
    - `node_created`: add node to `goalTree.nodes` if same goal
    - `node_status_changed`: update matching node's status in `goalTree`, `tasks`
    - `edge_created`: add edge to `goalTree.edges`

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add graph reactive store`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Create agents and search stores

**Files:**
- Create: `web/src/stores/agents.svelte.ts`
- Create: `web/src/stores/search.svelte.ts`

**Implementation:**

**agents.svelte.ts:**

- `agentsState` — `$state`:
  - `activeAgents: ActiveAgent[]`
  - `eventFeed: WsEvent[]` (ring buffer, max 200 items — newest at index 0)
  - `loading: boolean`

- Action functions:
  - `async loadAgents(goalId: string)` — calls `listAgents()`
  - `addEvent(event: WsEvent)` — prepends to `eventFeed`, trims to 200 max
  - `clearFeed()` — empties `eventFeed`

- WebSocket integration:
  - `handleWsEvent(event: WsEvent)` — for agent-related events (`agent_spawned`, `agent_progress`, `agent_completed`, `tool_execution`), add to `eventFeed`. For `agent_completed`, remove from `activeAgents`. For `agent_spawned`, add a placeholder to `activeAgents`.

**search.svelte.ts:**

- `searchState` — `$state`:
  - `query: string` (empty string default)
  - `results: GraphNode[]`
  - `nodeTypeFilter: NodeType | null`
  - `loading: boolean`
  - `error: string | null`

- Action functions:
  - `async executeSearch(projectId: string)` — calls `searchNodes()` with current `query`, `nodeTypeFilter`, limit 50
  - `setQuery(q: string)` — updates `query`
  - `setNodeTypeFilter(type: NodeType | null)` — updates filter
  - `clearSearch()` — resets all state

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add agents and search reactive stores`

<!-- END_TASK_3 -->

<!-- END_SUBCOMPONENT_A -->

<!-- START_TASK_4 -->
### Task 4: Create hash-based router

**Files:**
- Create: `web/src/router.svelte.ts`

**Implementation:**

Create a simple hash-based router using Svelte 5 runes. No external dependency.

- `routerState` — `$state`:
  - `path: string` (current hash path, e.g., `/projects`, `/goals/ra-abcd/tasks`)
  - `params: Record<string, string>` (extracted route parameters)

- Route definitions — an array of `{ pattern: string, name: string }` objects:
  - `/` → `'dashboard'`
  - `/projects` → `'project-list'`
  - `/projects/:projectId` → `'project-detail'`
  - `/goals/:goalId/tasks` → `'task-tree'`
  - `/goals/:goalId/decisions` → `'decision-graph'`
  - `/goals/:goalId/agents` → `'agent-monitor'`
  - `/goals/:goalId/sessions` → `'session-history'`
  - `/search` → `'graph-search'`

- `currentRoute` — `$derived`: match `path` against route patterns, return `{ name, params }` or `{ name: 'not-found', params: {} }`.

- `navigate(path: string)` — sets `window.location.hash = '#' + path`.

- Initialization: In an `$effect`, listen for `hashchange` events on `window`. Parse `window.location.hash` (strip leading `#`), match against patterns, update `routerState`. **Important:** The `$effect` must return a cleanup function that removes the event listener:

```typescript
$effect(() => {
  const handler = () => { /* parse hash and update routerState */ };
  handler(); // parse current hash on init
  window.addEventListener('hashchange', handler);
  return () => window.removeEventListener('hashchange', handler);
});
```

- Route pattern matching: Simple regex — convert `:param` segments to named capture groups. For example, `/goals/:goalId/tasks` becomes `/^\/goals\/([^/]+)\/tasks$/`. **Important:** Extract the route matching logic (`matchRoute(path, routes)`) as a pure exported function (not dependent on `$state`) so it can be unit tested directly.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add hash-based SPA router`

<!-- END_TASK_4 -->

<!-- START_TASK_5 -->
### Task 5: Build App shell with navigation and view routing

**Files:**
- Modify: `web/src/App.svelte`
- Create: `web/src/components/Sidebar.svelte`
- Create: `web/src/views/Placeholder.svelte`

**Implementation:**

**App.svelte:** Replace the placeholder with the full app shell:

- Import the router (`currentRoute`, `navigate` from `router.svelte.ts`).
- Import the WebSocket connection (`createWsConnection` from `api/websocket.ts`).
- Import stores for WebSocket event dispatch.
- On mount (`$effect`): initialize WebSocket connection, register a listener that dispatches events to all stores' `handleWsEvent` functions.
- Layout: A sidebar on the left (fixed width, ~240px) and main content area. Use CSS flexbox.
- The main content area renders the view component matching `currentRoute.name`. Use an `{#if}`/`{:else if}` chain:
  - `dashboard` → `<Placeholder name="Dashboard" />`
  - `project-list` → `<Placeholder name="Project List" />`
  - `project-detail` → `<Placeholder name="Project Detail" />`
  - `task-tree` → `<Placeholder name="Task Tree" />`
  - `decision-graph` → `<Placeholder name="Decision Graph" />`
  - `agent-monitor` → `<Placeholder name="Agent Monitor" />`
  - `session-history` → `<Placeholder name="Session History" />`
  - `graph-search` → `<Placeholder name="Graph Search" />`
  - default → `<Placeholder name="Not Found" />`

Actual view components will replace placeholders in Phases 4-7.

**Sidebar.svelte:** Navigation component with links:
- "Dashboard" → `/#/`
- "Projects" → `/#/projects`
- "Search" → `/#/search`
- A project selector (dropdown or list) that shows loaded projects. When a project is selected, show sub-links for that project's goals.
- Use `onclick` handlers that call `navigate()`. Mark the active link with a CSS class.

**Placeholder.svelte:** Simple component that accepts a `name` prop and renders `<div><h2>{name}</h2><p>Coming soon...</p></div>`. This lets us verify routing works before building real views.

**Styling:** Dark theme consistent with global.css. Sidebar: dark background (#12122a), nav links with hover/active states. Main content: slightly lighter (#1a1a2e), padded.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Dev server starts. Browser shows sidebar with navigation links. Clicking links changes the URL hash and renders the correct placeholder. Back/forward buttons work.

**Commit:** `feat(web): add app shell with sidebar navigation and routing`

<!-- END_TASK_5 -->

<!-- START_TASK_6 -->
### Task 6: Add unit tests for router pattern matching

**Verifies:** P4c.AC2.2 (hash routing maps URL fragments to view components)

**Files:**
- Create: `web/src/router.test.ts`

**Implementation:**

Test the pure `matchRoute` function exported from the router module. This function takes a path string and the route definitions, and returns `{ name, params }`.

**Testing:**
- P4c.AC2.2: Verify route matching produces correct results:
  - `/` matches `dashboard` with no params
  - `/projects` matches `project-list` with no params
  - `/projects/ra-abcd` matches `project-detail` with `{ projectId: 'ra-abcd' }`
  - `/goals/ra-1234/tasks` matches `task-tree` with `{ goalId: 'ra-1234' }`
  - `/goals/ra-1234/decisions` matches `decision-graph` with `{ goalId: 'ra-1234' }`
  - `/goals/ra-1234/agents` matches `agent-monitor` with `{ goalId: 'ra-1234' }`
  - `/goals/ra-1234/sessions` matches `session-history` with `{ goalId: 'ra-1234' }`
  - `/search` matches `graph-search` with no params
  - `/unknown/path` returns `not-found`
  - Empty string returns `dashboard` (root route)

**Verification:**

Run: `cd web && bun run test`
Expected: All tests pass.

**Commit:** `test(web): add unit tests for router pattern matching`

<!-- END_TASK_6 -->
