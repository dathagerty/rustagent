# Rustagent V2 Phase 4e: Task Tree View

**Goal:** Build the Task Tree view — a hierarchical visualization of goal → tasks → subtasks with dependency edges, status colors, agent assignments, and expandable/collapsible nodes.

**Architecture:** The Task Tree is a projection of the work graph that filters to task-type nodes and their hierarchical relationships. It uses the `GET /api/goals/:id/tree` endpoint which returns all nodes and edges under a goal. The view renders a tree using nested HTML elements (not Cytoscape — that's for the Decision Graph). Task status drives visual styling. The view also shows ready tasks and the recommended next task.

**Tech Stack:** Svelte 5 (runes), TypeScript, CSS

**Scope:** Phase 5 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4e.AC1: Tree rendering
- **P4e.AC1.1 Success:** TaskTree renders a hierarchical tree of goal → tasks → subtasks based on `Contains` edges
- **P4e.AC1.2 Success:** Each node shows title, status badge, priority badge, and assigned agent (if any)
- **P4e.AC1.3 Success:** Tree nodes are expandable/collapsible (toggle children visibility)

### P4e.AC2: Task status visualization
- **P4e.AC2.1 Success:** Status badges use colors matching the status (completed=green, in_progress=blue, blocked=red, etc.)
- **P4e.AC2.2 Success:** Dependency edges are shown as visual indicators (e.g., "depends on: ra-xxxx.1")
- **P4e.AC2.3 Success:** Ready tasks are visually highlighted

### P4e.AC3: Task views integration
- **P4e.AC3.1 Success:** "Ready Tasks" panel shows tasks from `GET /api/goals/:id/tasks/ready`
- **P4e.AC3.2 Success:** "Next Task" recommendation shown from `GET /api/goals/:id/tasks/next`
- **P4e.AC3.3 Success:** Clicking a node shows its detail (description, acceptance criteria, metadata) in a side panel

---

<!-- START_TASK_1 -->
### Task 1: Create GraphNodeCard component

**Files:**
- Create: `web/src/components/GraphNodeCard.svelte`

**Implementation:**

A reusable card component for displaying a graph node's information. Accepts a `node: GraphNode` prop.

**Renders:**
- Title (as heading)
- Status badge (using `StatusBadge` component)
- Priority badge (using `PriorityBadge` component)
- Node type label (small, muted text)
- Assigned agent (if `assigned_to` is set, show as a chip)
- Blocked reason (if `blocked_reason` is set, show in red)
- Description (truncated to 2-3 lines with "show more" toggle)
- Metadata display: If `metadata.acceptance_criteria` exists, render as a checklist. Other metadata as key-value pairs.
- Timestamps: created_at (always), started_at, completed_at (if set), formatted as relative time (e.g., "2 hours ago") or date.

Accept an `onclick` prop for click handling. Add a subtle hover effect.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add GraphNodeCard component`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Build tree data transformation

**Files:**
- Create: `web/src/lib/tree.ts`

**Implementation:**

Create utility functions that transform the flat `GoalTree` (nodes + edges) response into a nested tree structure suitable for rendering.

```typescript
interface TreeNode {
  node: GraphNode;
  children: TreeNode[];
  dependencies: GraphNode[];  // nodes this one depends on (via edges where edge_type === 'dependson')
  expanded: boolean;
}
```

- `buildTree(goalTree: GoalTree): TreeNode` — takes the flat nodes/edges from the API, builds a nested tree:
  1. Find the root node (the goal node, the one with no incoming `Contains` edges).
  2. For each node, find its children: nodes where there's a `Contains` edge from parent to child.
  3. Sort children by ID (natural sort on the dotted hierarchy — `ra-xxxx.1` before `ra-xxxx.2`).
  4. For each node, find dependencies: nodes connected via edges where `edge_type === 'dependson'` (from the current node to the dependency).
  5. Return the root `TreeNode` with nested children.

- `flattenTree(root: TreeNode): TreeNode[]` — depth-first flattened list for rendering (useful for keyboard navigation).

- `countTaskStats(root: TreeNode): { total: number; completed: number; inProgress: number; blocked: number; ready: number }` — recursively count task nodes by status.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add tree data transformation utilities`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Add unit tests for tree transformation utilities

**Verifies:** P4e.AC1.1 (hierarchical tree from Contains edges), P4e.AC2.1 (status-based task counting)

**Files:**
- Create: `web/src/lib/tree.test.ts`

**Implementation:**

Test the pure functions from `tree.ts`: `buildTree`, `flattenTree`, and `countTaskStats`.

Create test fixtures: a small set of `GraphNode[]` and `GraphEdge[]` representing a goal with 2 tasks (one completed, one in_progress) and a subtask under the first task. Include `Contains` edges for hierarchy and a `dependson` edge for dependencies.

**Testing:**
- P4e.AC1.1: `buildTree` produces correct nesting — root is the goal, children are tasks, subtask is nested under its parent. Children sorted by ID.
- P4e.AC1.1: `buildTree` correctly identifies dependencies from `dependson` edges.
- P4e.AC1.1: `flattenTree` returns nodes in depth-first order.
- P4e.AC2.1: `countTaskStats` returns correct counts — e.g., `{ total: 3, completed: 1, inProgress: 1, blocked: 0, ready: 1 }` for the fixture data.
- Edge case: empty nodes/edges array produces a sensible result (or throws descriptively).

**Verification:**

Run: `cd web && bun run test`
Expected: All tests pass.

**Commit:** `test(web): add unit tests for tree transformation utilities`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Build TaskTree view

**Files:**
- Create: `web/src/views/TaskTree.svelte`
- Create: `web/src/components/TreeNodeRow.svelte`

**Implementation:**

**TaskTree.svelte:** The main view component for the task tree route.

**Params:** Extract `goalId` from `currentRoute.params`.

**On mount/param change** (`$effect`):
- Call `loadGoalTree(goalId)` from the graph store
- Call `loadReadyTasks(goalId)` from the graph store
- Call `getNextTask(goalId)` from the API client directly (store the result locally)

**Layout:** Three-panel layout:

1. **Left panel (tree):** The hierarchical tree view. Render using recursive `TreeNodeRow` components. Show a progress bar at the top (completed tasks / total tasks).

2. **Right panel (detail):** When a node is selected, show its `GraphNodeCard` with full details. Include:
   - Description
   - Acceptance criteria (from metadata)
   - Dependencies list (linked, clickable)
   - Incoming/outgoing edges (from `getNode()` response)

3. **Bottom panel (ready tasks):** A horizontal list/bar showing ready tasks with a highlight on the "Next recommended" task.

**TreeNodeRow.svelte:** Renders a single row in the tree:
- Indentation based on depth (use `padding-left: depth * 24px`)
- Expand/collapse toggle (arrow icon or +/- button) if node has children
- Node title
- Status badge
- Priority badge
- Assigned agent chip (if set)
- Dependency indicator: if node has edges where `edge_type === 'dependson'`, show a small "blocked by" or "depends on" label
- Ready highlight: if status is "ready", add a distinct background or border color
- Click handler: select the node (shows detail in right panel)

**Styling:** Monospace/condensed font for the tree. Alternating row backgrounds. Indentation lines (CSS border-left on nested levels).

Update `App.svelte` to import and render `TaskTree` for the `task-tree` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Navigating to `/#/goals/<goalId>/tasks` shows the task tree. Nodes expand/collapse. Clicking a node shows its detail. Ready tasks panel shows at bottom.

**Commit:** `feat(web): add TaskTree view with hierarchical rendering and detail panel`

<!-- END_TASK_4 -->
