# Rustagent V2 Phase 4g: Agent Monitor + Session History + Graph Search

**Goal:** Build the remaining three views: Agent Monitor (real-time agent activity feed via WebSocket), Session History (past sessions with handoff notes), and Graph Search (FTS5 full-text search with filtering).

**Architecture:** The Agent Monitor consumes WebSocket events (`agent_spawned`, `agent_progress`, `agent_completed`, `tool_execution`) to show a real-time activity feed — like watching terminal sessions. Session History queries the sessions API to show past sessions with handoff notes for context continuity. Graph Search uses the FTS5 search endpoint to find nodes by title/description with optional type filtering.

**Tech Stack:** Svelte 5 (runes), TypeScript

**Scope:** Phase 7 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4g.AC1: Agent Monitor
- **P4g.AC1.1 Success:** AgentMonitor shows a real-time feed of WebSocket events as they arrive
- **P4g.AC1.2 Success:** Agent status badges show current state (spawning, working, completed)
- **P4g.AC1.3 Success:** Tool execution events show the tool name, arguments summary, and result summary
- **P4g.AC1.4 Success:** Feed auto-scrolls to newest events and caps at 200 items

### P4g.AC2: Session History
- **P4g.AC2.1 Success:** SessionHistory loads and displays sessions for a goal, sorted newest-first
- **P4g.AC2.2 Success:** Each session shows start/end timestamps, participating agents, and summary
- **P4g.AC2.3 Success:** Handoff notes are displayed in a formatted markdown-like view (preformatted text with section headers)

### P4g.AC3: Graph Search
- **P4g.AC3.1 Success:** GraphSearch accepts a text query and displays matching nodes
- **P4g.AC3.2 Success:** Results can be filtered by node type (goal, task, decision, etc.)
- **P4g.AC3.3 Success:** Search results show node title, type badge, status badge, and description snippet
- **P4g.AC3.4 Success:** Clicking a search result navigates to the appropriate view for that node type

---

<!-- START_TASK_1 -->
### Task 1: Build AgentStatusBadge component

**Files:**
- Create: `web/src/components/AgentStatusBadge.svelte`

**Implementation:**

Accepts `agentId: string` and `status: string` props. Renders a colored badge with the agent ID (truncated to last 6 chars) and status indicator.

Status colors:
- `spawning`/`initializing` → pulsing blue
- `working` → animated green (spinning or pulsing border)
- `reporting` → yellow
- `completed` → solid green
- `failed` → solid red

The badge should include a small icon or dot indicating the status alongside the text.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add AgentStatusBadge component`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Build AgentMonitor view

**Files:**
- Create: `web/src/views/AgentMonitor.svelte`

**Implementation:**

The Agent Monitor shows a real-time feed of agent activity for a goal, driven by WebSocket events from the agents store.

**Params:** Extract `goalId` from `currentRoute.params`.

**On mount** (`$effect`):
- Call `loadAgents(goalId)` from the agents store to get currently active agents.
- The agents store's `eventFeed` is already populated by the WebSocket handler in App.svelte.

**Layout:**

1. **Active agents bar (top):** Horizontal row of `AgentStatusBadge` components for each active agent. Shows count: "3 agents running".

2. **Event feed (main area):** A scrollable list of events from `agentsState.eventFeed`. Each event renders differently based on type:

   - `agent_spawned`: "[time] Agent {agent_id} spawned with profile {profile} for goal {goal_id}" — blue background
   - `agent_progress`: "[time] Agent {agent_id} (turn {turn}): {summary}" — default background
   - `agent_completed`: "[time] Agent {agent_id} completed: {summary} ({tokens_used} tokens)" — green background (or red if outcome indicates failure)
   - `tool_execution`: "[time] Agent {agent_id} → {tool}({args summary}) = {result summary}" — monospace font, muted background. Truncate long args/results with expandable toggle.
   - Other events: "[time] {type}: {JSON.stringify}" — gray, muted

3. **Auto-scroll:** The feed container should auto-scroll to the bottom when new events arrive. Use an `$effect` that watches `agentsState.eventFeed.length` and scrolls the container to `scrollHeight`. Provide a "pause auto-scroll" toggle for when the user is reading older events.

4. **Clear button:** Button to call `clearFeed()` from the agents store.

**Time formatting:** Show events with relative time ("2s ago", "1m ago") that updates periodically (every 10s via `setInterval` in an `$effect`).

Update `App.svelte` to import and render `AgentMonitor` for the `agent-monitor` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Navigating to `/#/goals/<goalId>/agents` shows the agent monitor. If the daemon is running agents, events appear in real-time. Otherwise the feed is empty with a "No agent activity" message.

**Commit:** `feat(web): add AgentMonitor view with real-time event feed`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Build SessionHistory view

**Files:**
- Create: `web/src/views/SessionHistory.svelte`

**Implementation:**

Shows past sessions for a goal with handoff notes.

**Params:** Extract `goalId` from `currentRoute.params`.

**On mount/param change** (`$effect`): Call `listSessions(goalId)` from the API client. Store results locally sorted by `started_at` descending (newest first).

**Layout:**

1. **Sessions list:** An accordion or card list. Each session card shows:
   - Session ID (truncated)
   - Start time → End time (formatted as dates, or "In progress" if `ended_at` is null)
   - Duration (calculated from start/end)
   - Participating agents (from `agent_ids` array, rendered as chips)
   - Summary (if available, shown as subtitle)

2. **Expanded session:** When a session card is clicked/expanded:
   - Full handoff notes rendered in a `<pre>` block with monospace font. The handoff notes use a Markdown-like format with `## Done`, `## Remaining`, `## Blocked`, `## Decisions Made` sections. Render as preformatted text with some basic styling (bold section headers).
   - If no handoff notes: "No handoff notes for this session."

3. **Empty state:** "No sessions found for this goal."

Update `App.svelte` to import and render `SessionHistory` for the `session-history` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

**Commit:** `feat(web): add SessionHistory view with handoff notes display`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Build SearchResult component and GraphSearch view

**Files:**
- Create: `web/src/components/SearchResult.svelte`
- Create: `web/src/views/GraphSearch.svelte`

**Implementation:**

**SearchResult.svelte:** Accepts `node: GraphNode` prop. Renders a compact card showing:
- Node type badge (colored chip: "goal", "task", "decision", etc.)
- Title (as a clickable link)
- Status badge
- Description snippet (first 150 chars, with ellipsis)
- Node ID (muted, small text)

The click handler navigates to the appropriate view based on node type:
- `goal` or `task` → `/#/goals/{rootGoalId}/tasks` (extract root goal ID from the node's hierarchical ID — everything before the first dot, or the ID itself if no dot)
- `decision` or `option` → `/#/goals/{rootGoalId}/decisions`
- `observation` or `outcome` or `revisit` → `/#/goals/{rootGoalId}/tasks` (general view)

**GraphSearch.svelte:** The search view.

**State** (from search store):
- `searchState.query`, `searchState.results`, `searchState.nodeTypeFilter`, `searchState.loading`, `searchState.error`

**Layout:**

1. **Search bar (top):** Text input bound to `searchState.query`. Search button (or Enter key) triggers `executeSearch(projectId)`. Debounced input not needed — explicit search trigger.

2. **Filter bar:** Row of toggle buttons for node type filtering: "All", "Goals", "Tasks", "Decisions", "Options", "Outcomes", "Observations". Clicking sets `nodeTypeFilter` and re-executes search.

3. **Results list:** Render `SearchResult` for each result in `searchState.results`. Show result count: "42 results for 'authentication'".

4. **Empty states:**
   - No query yet: "Enter a search term to find nodes across the work graph."
   - No results: "No nodes found matching '{query}'."

**Integration:** The search requires a `projectId`. Get this from the projects store's `selectedProjectId`. If no project is selected, prompt the user to select one first.

Update `App.svelte` to import and render `GraphSearch` for the `graph-search` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Navigating to `/#/search` shows the search interface. Entering a query and clicking search returns matching nodes. Type filter toggles work. Clicking a result navigates to the appropriate view.

**Commit:** `feat(web): add GraphSearch view with FTS5 search and type filtering`

<!-- END_TASK_4 -->
