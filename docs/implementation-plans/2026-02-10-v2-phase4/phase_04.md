# Rustagent V2 Phase 4d: Dashboard + Project Views

**Goal:** Build the Dashboard overview, Project List, and Project Detail views that form the primary entry points for navigating the work graph.

**Architecture:** The Dashboard shows an at-a-glance overview of all active goals across projects, running agents, and recent activity. ProjectList shows all registered projects. ProjectDetail shows goals, task summary, and decision summary for a selected project. All views fetch data from the daemon API via the stores created in Phase 3.

**Tech Stack:** Svelte 5 (runes), TypeScript

**Scope:** Phase 4 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4d.AC1: Dashboard view
- **P4d.AC1.1 Success:** Dashboard loads and displays all projects with their goal counts
- **P4d.AC1.2 Success:** Dashboard shows active goals across all projects with status badges
- **P4d.AC1.3 Success:** Dashboard shows a summary of running agents (count and current tasks)

### P4d.AC2: Project List view
- **P4d.AC2.1 Success:** ProjectList loads and displays all registered projects with name, path, and registration date
- **P4d.AC2.2 Success:** Clicking a project navigates to ProjectDetail view

### P4d.AC3: Project Detail view
- **P4d.AC3.1 Success:** ProjectDetail loads goals for the selected project
- **P4d.AC3.2 Success:** Each goal shows title, status, priority, and task completion percentage
- **P4d.AC3.3 Success:** Navigation links from ProjectDetail lead to TaskTree, DecisionGraph, AgentMonitor, and SessionHistory for each goal

---

<!-- START_TASK_1 -->
### Task 1: Create shared UI components

**Files:**
- Create: `web/src/components/StatusBadge.svelte`
- Create: `web/src/components/PriorityBadge.svelte`
- Create: `web/src/components/LoadingSpinner.svelte`
- Create: `web/src/components/ErrorMessage.svelte`

**Implementation:**

**StatusBadge.svelte:** Accepts `status: NodeStatus` prop. Renders a colored pill/badge. Color mapping:
- `completed` → green
- `active`/`in_progress` → blue
- `ready`/`claimed` → teal
- `pending` → gray
- `blocked`/`failed` → red
- `decided`/`chosen` → purple
- `rejected`/`abandoned`/`superseded`/`cancelled` → muted/orange
- `review` → yellow

Use a `<span>` with inline-block, border-radius, padding, and background-color from a CSS variable or direct color map.

**PriorityBadge.svelte:** Accepts `priority: Priority | null` prop. Renders nothing if null. Colored indicator:
- `critical` → red
- `high` → orange
- `medium` → yellow
- `low` → gray

**LoadingSpinner.svelte:** A simple CSS spinner animation. Accepts optional `size` prop (default: `24px`).

**ErrorMessage.svelte:** Accepts `message: string` prop. Renders a styled error box with the message.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add shared StatusBadge, PriorityBadge, LoadingSpinner, ErrorMessage components`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Build Dashboard view

**Files:**
- Create: `web/src/views/Dashboard.svelte`

**Implementation:**

Replace the Dashboard placeholder in App.svelte with the real component.

**On mount** (`$effect`): Call `loadProjects()` from the projects store. For each project, fetch its goals via `listGoals(project.id)` (directly from API client, since the graph store is goal-scoped).

**Layout:** A grid of cards:

1. **Projects summary card**: Shows total project count. Lists each project name with its goal count as a clickable link to ProjectDetail.

2. **Active goals card**: Lists all goals with status `active` across all projects. Each shows: title, project name, status badge, priority badge. Clicking navigates to the goal's task tree.

3. **Agents card**: Shows total active agents count. If a specific goal is in the URL or there's an active goal, show the agents for it. Otherwise show a message like "Select a goal to see agents."

Handle loading state (show `<LoadingSpinner />`) and error state (show `<ErrorMessage />`).

**Styling:** CSS grid with responsive columns. Cards have a dark background (#242444), border-radius, padding, subtle border.

Update `App.svelte` to import and render `Dashboard` instead of `<Placeholder name="Dashboard" />` for the `dashboard` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Dashboard renders at `/#/`. If daemon is running with projects/goals, they appear. If daemon is not running, error message shows.

**Commit:** `feat(web): add Dashboard view with projects and goals overview`

<!-- END_TASK_2 -->

<!-- START_TASK_3 -->
### Task 3: Build ProjectList view

**Files:**
- Create: `web/src/views/ProjectList.svelte`

**Implementation:**

Replace the ProjectList placeholder.

**On mount**: Call `loadProjects()` from the projects store.

**Layout:** A table or list of projects. Each row shows:
- Project name (clickable, navigates to `/#/projects/${project.id}`)
- Path (displayed as monospace text)
- Registered date (formatted as a human-readable date)

Handle empty state: "No projects registered. Use `rustagent project add <name> <path>` to register a project."

Handle loading/error states.

Update `App.svelte` to import and render `ProjectList` for the `project-list` route.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

**Commit:** `feat(web): add ProjectList view`

<!-- END_TASK_3 -->

<!-- START_TASK_4 -->
### Task 4: Build ProjectDetail view

**Files:**
- Create: `web/src/views/ProjectDetail.svelte`

**Implementation:**

Replace the ProjectDetail placeholder.

**Props/params**: Extract `projectId` from `currentRoute.params`.

**On mount/param change** (`$effect`): Load the project details (`getProject(projectId)`) and goals (`listGoals(projectId)`). Also load active decisions (`listDecisions(projectId)`).

**Layout:**

1. **Header**: Project name, path, registration date.

2. **Goals section**: A list/grid of goal cards. Each goal card shows:
   - Title, status badge, priority badge
   - Task summary: count of completed/total tasks (calculate from the goal tree — fetch per goal with `getGoalTree(goalId)`, count nodes where `node_type === 'task'`)
   - Action links: "Tasks" → `/#/goals/${goalId}/tasks`, "Decisions" → `/#/goals/${goalId}/decisions`, "Agents" → `/#/goals/${goalId}/agents`, "Sessions" → `/#/goals/${goalId}/sessions`

3. **Active decisions section**: List of decision nodes with status badges. Clicking navigates to the decision graph.

Handle empty state (no goals), loading, error.

Update `App.svelte` to import and render `ProjectDetail` for the `project-detail` route, passing `currentRoute.params.projectId`.

**Verification:**

Run: `cd web && bun run build`
Expected: Build succeeds.

Run: `cd web && bun run dev`
Expected: Navigating to `/#/projects/<id>` shows project details with goals. Goal action links navigate to correct routes.

**Commit:** `feat(web): add ProjectDetail view with goals and decisions`

<!-- END_TASK_4 -->
