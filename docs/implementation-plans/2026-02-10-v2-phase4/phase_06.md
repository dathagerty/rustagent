# Rustagent V2 Phase 4f: Decision Graph View

**Goal:** Build the Decision Graph view using Cytoscape.js for interactive graph visualization of decision/option/outcome/revisit nodes with now/history mode toggle.

**Architecture:** The Decision Graph is a projection of the work graph that filters to decision-related node types. It uses Cytoscape.js (with the dagre layout extension) for pan/zoom/drag graph rendering. The view supports two modes from the Deciduous pattern: "Now" mode (active decisions only) and "History" mode (full evolution including abandoned/superseded paths). Nodes are styled by type (decision=diamond, option=hexagon, outcome=ellipse, etc.).

**Tech Stack:** Svelte 5 (runes), TypeScript, Cytoscape.js 3.x, cytoscape-dagre

**Scope:** Phase 6 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4f.AC1: Cytoscape integration
- **P4f.AC1.1 Success:** CytoscapeGraph wrapper component initializes Cytoscape.js with a container div and properly destroys on unmount
- **P4f.AC1.2 Success:** Dagre layout renders nodes in a top-to-bottom DAG hierarchy
- **P4f.AC1.3 Success:** Pan, zoom, and drag interactions work in the graph view

### P4f.AC2: Decision graph rendering
- **P4f.AC2.1 Success:** Decision nodes render as diamonds (yellow/gold)
- **P4f.AC2.2 Success:** Option nodes render as hexagons (chosen=green, rejected=muted/gray)
- **P4f.AC2.3 Success:** Outcome and Revisit nodes render with distinct shapes and colors
- **P4f.AC2.4 Success:** Edge labels show relationship types (chosen, rejected with reason, leads_to, etc.)

### P4f.AC3: Now/History modes
- **P4f.AC3.1 Success:** "Now" mode shows only active/decided decisions and chosen options (from `GET /api/projects/:id/decisions`)
- **P4f.AC3.2 Success:** "History" mode shows the full decision evolution including rejected/abandoned/superseded nodes (from `GET /api/projects/:id/decisions/history`)
- **P4f.AC3.3 Success:** Toggle between modes re-renders the graph with appropriate filtering

### P4f.AC4: Node interaction
- **P4f.AC4.1 Success:** Clicking a node shows its detail in a side panel (using GraphNodeCard)
- **P4f.AC4.2 Success:** Option nodes show pros/cons from metadata
- **P4f.AC4.3 Success:** "Fit to view" button centers and zooms the graph to show all nodes

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Create CytoscapeGraph wrapper component

**Files:**
- Modify: `web/package.json` (add cytoscape dependencies)
- Create: `web/src/components/CytoscapeGraph.svelte`

**Implementation:**

First, install Cytoscape.js dependencies:

Run: `cd web && bun add cytoscape && bun add -d @types/cytoscape cytoscape-dagre`

This adds `cytoscape` as a runtime dependency and `@types/cytoscape` + `cytoscape-dagre` as dev dependencies.

A reusable Svelte 5 wrapper around Cytoscape.js. This component handles initialization, reactive updates, cleanup, and exposes the instance for external control.

**Props** (using `$props()`):
- `elements: cytoscape.ElementDefinition[]` — nodes and edges data
- `stylesheet: cytoscape.Stylesheet[]` — visual styling rules
- `layoutOptions: object` — layout configuration (default: `{ name: 'dagre', rankDir: 'TB', spacingFactor: 1.5 }`)
- `onNodeClick: (nodeId: string) => void` — callback when a node is tapped
- `onBackgroundClick: () => void` — callback when background is tapped (deselect)

**Lifecycle:**
- Use `let containerDiv: HTMLDivElement` with `bind:this`.
- In `onMount()` (from `'svelte'`): Initialize `cytoscape({...})` with the container, elements, stylesheet, and layout. Register `cy.on('tap', 'node', ...)` and `cy.on('tap', ...)` event handlers. Register the dagre extension: `cytoscape.use(dagre)` (do this once, at module level, before any instances are created). Return a cleanup function that calls `cy.destroy()`.

**Reactive updates** (`$effect`):
- When `elements` changes: `cy.batch(() => { cy.elements().remove(); cy.add(elements); })`, then re-run layout.
- When `stylesheet` changes: `cy.style(stylesheet)`.

**Performance optimizations:**
- `pixelRatio: 1` (reduce canvas resolution on high-DPI)
- `wheelSensitivity: 0.2` (comfortable zoom speed)

**Exported functions:**
- `fitView()` — calls `cy.fit(undefined, 20)` (20px padding)
- `getInstance()` — returns the `cy` instance for advanced use

Note: Import `cytoscape` and `cytoscape-dagre` at the module level. Register the dagre extension with `cytoscape.use(dagre)` once. The `import type` for Cytoscape types can come from `cytoscape` (ships types) or `@types/cytoscape`.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add CytoscapeGraph wrapper component with dagre layout`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Create graph data transformation for decisions

**Files:**
- Create: `web/src/lib/decision-graph.ts`

**Implementation:**

Utility functions that transform API responses into Cytoscape element definitions.

```typescript
import type { ElementDefinition } from 'cytoscape';
```

- `decisionsToElements(nodes: GraphNode[], edges: GraphEdge[]): ElementDefinition[]` — converts GraphNode/GraphEdge arrays to Cytoscape element format:
  - Each node becomes: `{ data: { id: node.id, label: node.title, type: node.node_type, status: node.status, ...other fields for styling } }`
  - Each edge becomes: `{ data: { id: edge.id, source: edge.from_node, target: edge.to_node, label: edge.label || edge.edge_type, edgeType: edge.edge_type } }`

- `filterNowMode(nodes: GraphNode[], edges: GraphEdge[]): { nodes: GraphNode[], edges: GraphEdge[] }` — filter for "Now" mode:
  - Include Decision nodes with status `active` or `decided`
  - Include Option nodes with status `chosen`
  - Include Outcome nodes with status `active` or `completed`
  - Exclude: `rejected`, `abandoned`, `superseded` nodes
  - Include only edges where both endpoints are in the filtered set

- `decisionStylesheet(): cytoscape.Stylesheet[]` — returns the Cytoscape stylesheet with node-type styling:
  - Decision: diamond shape, gold (#FFD700) background, dark border
  - Option (chosen): hexagon shape, green (#32CD32) background
  - Option (rejected): hexagon shape, gray (#666) background, dashed border
  - Option (abandoned): hexagon shape, muted red, dashed border
  - Outcome (completed): ellipse, green
  - Outcome (active): ellipse, blue
  - Revisit: triangle, orange
  - Edges: straight lines, arrow at target, label from data
  - Chosen edges: thicker, green
  - Rejected edges: dashed, gray, with reason label
  - Selected node: highlighted border

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add decision graph data transformation and stylesheet`

<!-- END_TASK_2 -->

<!-- END_SUBCOMPONENT_A -->

<!-- START_TASK_3 -->
### Task 3: Build DecisionGraph view

**Files:**
- Create: `web/src/views/DecisionGraph.svelte`

**Implementation:**

The main Decision Graph view component.

**Params:** Extract `goalId` from `currentRoute.params`. Derive `projectId` from the selected project in the projects store.

**State:**
- `mode: 'now' | 'history'` — `$state('now')` (default Now mode)
- `selectedNodeId: string | null`
- `selectedNodeDetail: GraphNode | null`

**On mount/param change** (`$effect`):
- If `mode === 'now'`: call `listDecisions(projectId)` to get active decisions. Also fetch related option/outcome nodes by loading the goal tree and filtering.
- If `mode === 'history'`: call `getDecisionHistory(projectId)` to get all decision-related nodes and edges.
- Transform the response using `decisionsToElements()` (and `filterNowMode()` for Now mode).
- Pass elements and stylesheet to the `CytoscapeGraph` component.

**Layout:**

1. **Toolbar (top):** Mode toggle buttons ("Now" / "History") with active state styling. "Fit to view" button. Node count display.

2. **Graph area (center):** The `CytoscapeGraph` component filling the available space. Must have explicit height (e.g., `calc(100vh - toolbar - detail)`).

3. **Detail panel (right or bottom):** When a node is clicked (`onNodeClick`), show `GraphNodeCard` with full details. For Option nodes, also display pros/cons from metadata as styled lists. For Decision nodes with chosen option, show which option was chosen and why.

**Mode toggle behavior:**
- Switching mode re-fetches data and re-renders the graph.
- Now mode: cleaner view showing current truth.
- History mode: full graph with rejected/abandoned paths visible (grayed out).

Update `App.svelte` to import and render `DecisionGraph` for the `decision-graph` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Navigating to `/#/goals/<goalId>/decisions` shows the decision graph. Nodes are styled by type. Now/History toggle works. Clicking nodes shows detail. Pan/zoom/drag work.

**Commit:** `feat(web): add DecisionGraph view with Cytoscape.js and Now/History modes`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Add unit tests for decision graph transformations

**Verifies:** P4f.AC2.1 (decision nodes as diamonds), P4f.AC2.2 (option nodes as hexagons), P4f.AC3.1 (Now mode filtering), P4f.AC3.2 (History mode shows full evolution)

**Files:**
- Create: `web/src/lib/decision-graph.test.ts`

**Implementation:**

Test the pure functions from `decision-graph.ts`: `decisionsToElements`, `filterNowMode`, and `decisionStylesheet`.

Create test fixtures: a set of `GraphNode[]` with decision, option (chosen + rejected), outcome, and revisit nodes. Create `GraphEdge[]` with chosen, rejected, and contains edges.

**Testing:**
- P4f.AC2.1, P4f.AC2.2: `decisionsToElements` converts nodes to Cytoscape elements with correct `data.type` and `data.status` fields that the stylesheet uses for styling.
- P4f.AC2.1, P4f.AC2.2: `decisionsToElements` converts edges to Cytoscape elements with correct `source`, `target`, and `edgeType` fields.
- P4f.AC3.1: `filterNowMode` includes active/decided decisions and chosen options, excludes rejected/abandoned/superseded nodes.
- P4f.AC3.1: `filterNowMode` excludes edges where either endpoint was filtered out.
- P4f.AC3.2: Without `filterNowMode`, all nodes including rejected/abandoned are present (History mode).
- Edge case: empty input arrays produce empty output.
- `decisionStylesheet` returns a non-empty stylesheet array.

**Verification:**

Run: `cd web && bun run test`
Expected: All tests pass.

**Commit:** `test(web): add unit tests for decision graph transformations`

<!-- END_TASK_4 -->
