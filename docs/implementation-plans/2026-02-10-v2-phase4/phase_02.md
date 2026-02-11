# Rustagent V2 Phase 4b: API Client + WebSocket Connection Handler

**Goal:** Build a typed HTTP API client and WebSocket connection handler in TypeScript that matches the daemon's REST API and WebSocket event types exactly.

**Architecture:** The API client wraps `fetch()` calls to the daemon's REST endpoints. The WebSocket handler manages a persistent connection to `/ws` with automatic reconnection and event dispatching via callbacks. TypeScript types mirror the Rust serde serialization format for `GraphNode`, `GraphEdge`, `Session`, `ProjectResponse`, and all `WsEvent` variants.

**Tech Stack:** TypeScript, native `fetch`, native `WebSocket`

**Scope:** Phase 2 of 8 from the v2 Phase 4 architecture (Web UI)

**Codebase verified:** 2026-02-10

**Design document:** `/Users/david.hagerty/code/personal/rustagent/new-directions/docs/plans/v2-architecture.md`

---

## Acceptance Criteria Coverage

This phase implements and verifies:

### P4b.AC1: TypeScript types
- **P4b.AC1.1 Success:** `types.ts` defines interfaces matching all Rust API response shapes: `GraphNode`, `GraphEdge`, `Session`, `ProjectResponse`, `NodeWithEdges`, `GoalTree`, `DecisionHistory`, `ExportResult`, `ImportResult`, `DiffResult`, `ActiveAgent`, `SearchRequest`
- **P4b.AC1.2 Success:** `WsEvent` discriminated union type covers all 9 event types with correct `type` field tags
- **P4b.AC1.3 Success:** Enum string literal types for `NodeType`, `EdgeType`, `NodeStatus`, `Priority` match Rust serde serialization

### P4b.AC2: HTTP API client
- **P4b.AC2.1 Success:** `ApiClient` class exposes typed methods for all 21+ REST endpoints
- **P4b.AC2.2 Success:** Error responses (4xx, 5xx) throw typed `ApiError` with status code and message
- **P4b.AC2.3 Success:** `ApiClient` uses a configurable base URL (default empty string for same-origin via Vite proxy)

### P4b.AC3: WebSocket handler
- **P4b.AC3.1 Success:** `WsConnection` connects to `/ws` and dispatches typed events to registered listeners
- **P4b.AC3.2 Success:** Automatic reconnection with exponential backoff on disconnect
- **P4b.AC3.3 Success:** Connection state exposed as reactive value (`connected: boolean`)

---

<!-- START_SUBCOMPONENT_A (tasks 1-2) -->

<!-- START_TASK_1 -->
### Task 1: Create TypeScript types matching Rust API

**Files:**
- Create: `web/src/types.ts`

**Implementation:**

Define TypeScript interfaces and type aliases that exactly match the daemon's JSON serialization. The field names, casing, and nullability must match what the Rust `serde` derives produce.

**Enum types** — use string literal unions matching Rust serde serialization:

- `NodeType` uses `#[serde(rename_all = "lowercase")]` — single-word variants, so lowercase is identical to snake_case.
- `EdgeType` uses `#[serde(rename_all = "lowercase")]` — **WARNING:** This means `DependsOn` serializes as `"dependson"` (not `"depends_on"`) and `LeadsTo` as `"leadsto"` (not `"leads_to"`). The Rust `Display` impl shows `depends_on`/`leads_to` but JSON serialization uses serde, not Display.
- `NodeStatus` uses `#[serde(rename_all = "snake_case")]` — multi-word variants get underscores (e.g., `in_progress`).
- `Priority` uses `#[serde(rename_all = "lowercase")]` — single-word variants.

```typescript
type NodeType = 'goal' | 'task' | 'decision' | 'option' | 'outcome' | 'observation' | 'revisit';
type EdgeType = 'contains' | 'dependson' | 'leadsto' | 'chosen' | 'rejected' | 'supersedes' | 'informs';
type NodeStatus = 'pending' | 'active' | 'completed' | 'cancelled' | 'ready' | 'claimed' | 'in_progress' | 'review' | 'blocked' | 'failed' | 'decided' | 'superseded' | 'abandoned' | 'chosen' | 'rejected';
type Priority = 'critical' | 'high' | 'medium' | 'low';
```

**Core types** — match the JSON shapes from the codebase investigation:

- `GraphNode`: 15 fields (`id`, `project_id`, `node_type`, `title`, `description`, `status`, `priority`, `assigned_to`, `created_by`, `labels`, `created_at`, `started_at`, `completed_at`, `blocked_reason`, `metadata`). Optional fields are `T | null`.
- `GraphEdge`: 6 fields (`id`, `edge_type`, `from_node`, `to_node`, `label`, `created_at`).
- `ProjectResponse`: 4 fields (`id`, `name`, `path`, `registered_at`).
- `Session`: 8 fields (`id`, `project_id`, `goal_id`, `started_at`, `ended_at`, `handoff_notes`, `agent_ids`, `summary`).
- `ActiveAgent`: 4 fields (`agent_id`, `task_id`, `task_title`, `task_status`).

**Composite response types:**

- `NodeWithEdges`: `{ node: GraphNode; incoming_edges: [GraphEdge, GraphNode][]; outgoing_edges: [GraphEdge, GraphNode][] }`
- `GoalTree`: `{ nodes: GraphNode[]; edges: GraphEdge[] }`
- `DecisionHistory`: `{ nodes: GraphNode[]; edges: GraphEdge[] }`
- `ExportResult`: `{ goal_id: string; toml: string }`
- `ImportResult`: `{ added_nodes: number; added_edges: number; conflicts: ImportConflict[]; skipped_edges: string[]; unchanged: number }`
- `ImportConflict`: `{ node_id: string; field: string; db_value: string; file_value: string }`
- `DiffResult`: `{ added_nodes: string[]; changed_nodes: [string, string[]][]; removed_nodes: string[]; added_edges: string[]; removed_edges: string[]; unchanged_nodes: number; unchanged_edges: number }`

**Request body types:**

- `CreateProjectRequest`: `{ name: string; path: string }`
- `CreateGoalRequest`: `{ title: string; description: string; priority?: Priority }`
- `UpdateNodeRequest`: `{ status?: string; title?: string; description?: string; blocked_reason?: string; metadata?: Record<string, string> }`
- `CreateChildRequest`: `{ node_type: NodeType; title: string; description: string; priority?: Priority; metadata?: Record<string, string> }`
- `CreateEdgeRequest`: `{ edge_type: EdgeType; from_node: string; to_node: string; label?: string }`
- `SearchRequest`: `{ query: string; node_type?: NodeType; limit?: number }`
- `ImportRequest`: `{ toml: string; strategy?: 'merge' | 'theirs' | 'ours' }`

**WsEvent** — discriminated union on the `type` field. The Rust enum uses `#[serde(tag = "type", rename_all = "snake_case")]`. Note: the design document uses camelCase names (e.g., `agentSpawned`) but the actual Rust serde output uses snake_case (e.g., `agent_spawned`). The types below match the actual serialization:

```typescript
type WsEvent =
  | { type: 'agent_spawned'; agent_id: string; profile: string; goal_id: string }
  | { type: 'agent_progress'; agent_id: string; turn: number; summary: string }
  | { type: 'agent_completed'; agent_id: string; outcome_type: string; summary: string; tokens_used: number | null }
  | { type: 'node_created'; parent_id: string | null; id: string; project_id: string; node_type: NodeType; title: string; description: string; status: NodeStatus; priority: Priority | null; assigned_to: string | null; created_by: string | null; labels: string[]; created_at: string; started_at: string | null; completed_at: string | null; blocked_reason: string | null; metadata: Record<string, string> }
  | { type: 'node_status_changed'; node_id: string; node_type: string; old_status: string; new_status: string }
  | { type: 'edge_created'; id: string; edge_type: EdgeType; from_node: string; to_node: string; label: string | null; created_at: string }
  | { type: 'session_ended'; session_id: string; handoff_notes: string | null }
  | { type: 'tool_execution'; agent_id: string; tool: string; args: unknown; result: string }
  | { type: 'orchestrator_state_changed'; goal_id: string; state: string };
```

For `node_created` and `edge_created`, the Rust code uses `#[serde(flatten)]` so the node/edge fields are spread into the event object alongside the `type` field. The `node_created` type above shows all GraphNode fields explicitly. For `edge_created`, the flattened fields are: `id`, `edge_type`, `from_node`, `to_node`, `label`, `created_at`.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add TypeScript types matching daemon API`

<!-- END_TASK_1 -->

<!-- START_TASK_2 -->
### Task 2: Build HTTP API client

**Files:**
- Create: `web/src/api/client.ts`

**Implementation:**

Create an `ApiClient` class (or collection of functions) that wraps `fetch()` calls. Use a configurable `baseUrl` parameter (default: `''` for same-origin, enabling Vite proxy in dev).

For each endpoint, create a typed async function. Pattern:

```typescript
async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const opts: RequestInit = {
    method,
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  const res = await fetch(`${baseUrl}${path}`, opts);
  if (!res.ok) {
    const text = await res.text();
    throw new ApiError(res.status, text);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}
```

Define `ApiError` class with `status: number` and `message: string`.

Implement all endpoints as exported functions. Group by resource:

**Projects:**
- `listProjects()` → GET `/api/projects` → `ProjectResponse[]`
- `createProject(req)` → POST `/api/projects` → `ProjectResponse`
- `getProject(id)` → GET `/api/projects/${id}` → `ProjectResponse`
- `deleteProject(id)` → DELETE `/api/projects/${id}` → `void`

**Goals:**
- `listGoals(projectId)` → GET `/api/projects/${projectId}/goals` → `GraphNode[]`
- `createGoal(projectId, req)` → POST `/api/projects/${projectId}/goals` → `GraphNode`

**Nodes:**
- `getNode(id)` → GET `/api/nodes/${id}` → `NodeWithEdges`
- `updateNode(id, req)` → PATCH `/api/nodes/${id}` → `GraphNode`
- `createChild(parentId, req)` → POST `/api/nodes/${parentId}/children` → `GraphNode`

**Edges:**
- `createEdge(req)` → POST `/api/edges` → `GraphEdge`
- `deleteEdge(id)` → DELETE `/api/edges/${id}` → `void`

**Goal tree and task views:**
- `getGoalTree(goalId)` → GET `/api/goals/${goalId}/tree` → `GoalTree`
- `listTasks(goalId)` → GET `/api/goals/${goalId}/tasks` → `GraphNode[]`
- `listReadyTasks(goalId)` → GET `/api/goals/${goalId}/tasks/ready` → `GraphNode[]`
- `getNextTask(goalId)` → GET `/api/goals/${goalId}/tasks/next` → `GraphNode | null`

**Decisions:**
- `listDecisions(projectId)` → GET `/api/projects/${projectId}/decisions` → `GraphNode[]`
- `getDecisionHistory(projectId)` → GET `/api/projects/${projectId}/decisions/history` → `DecisionHistory`
- `exportDecisions(projectId)` → POST `/api/projects/${projectId}/decisions/export` → `string[]`

**Graph import/export:**
- `exportAllGoals(projectId)` → GET `/api/projects/${projectId}/graph/export` → `ExportResult[]`
- `exportGoal(goalId)` → GET `/api/goals/${goalId}/export` → `ExportResult`
- `importGraph(projectId, req)` → POST `/api/projects/${projectId}/graph/import` → `ImportResult`
- `diffGraph(projectId, req)` → POST `/api/projects/${projectId}/graph/diff` → `DiffResult`

**Sessions:**
- `listSessions(goalId)` → GET `/api/goals/${goalId}/sessions` → `Session[]`
- `getSession(id)` → GET `/api/sessions/${id}` → `Session`

**Search:**
- `searchNodes(projectId, req)` → POST `/api/projects/${projectId}/search` → `GraphNode[]`

**Agents:**
- `listAgents(goalId)` → GET `/api/goals/${goalId}/agents` → `ActiveAgent[]`

**Health:**
- `healthCheck()` → GET `/api/health` → `{ status: string }`

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add typed HTTP API client for all daemon endpoints`

<!-- END_TASK_2 -->

<!-- END_SUBCOMPONENT_A -->

<!-- START_TASK_3 -->
### Task 3: Build WebSocket connection handler

**Files:**
- Create: `web/src/api/websocket.ts`

**Implementation:**

Create a `WsConnection` class that manages a WebSocket connection to the daemon:

- **Constructor**: Takes a URL (default: auto-detect from `window.location` — use `ws://` or `wss://` based on `location.protocol`, point to `/ws`).
- **Connection state**: `connected` boolean. Use a simple variable (will be wrapped in a reactive store in Phase 3).
- **Event listeners**: `onEvent(callback: (event: WsEvent) => void)` — registers a callback for all events. `offEvent(callback)` — unregisters.
- **Reconnection**: On `onclose` or `onerror`, attempt reconnection with exponential backoff: 1s, 2s, 4s, 8s, max 30s. Reset backoff on successful connection.
- **connect()**: Opens the WebSocket. Parses incoming messages as JSON, type-narrows to `WsEvent` based on the `type` field, dispatches to listeners.
- **disconnect()**: Closes the WebSocket, stops reconnection attempts.
- **Message parsing**: `JSON.parse(event.data)` — if it fails, log a warning and skip the message. If `type` field is unrecognized, log and skip (forward compatibility).

Export a singleton-style `createWsConnection()` factory function.

**Verification:**

Run: `cd web && npx tsc --noEmit`
Expected: No type errors.

**Commit:** `feat(web): add WebSocket connection handler with auto-reconnect`

<!-- END_TASK_3 -->
