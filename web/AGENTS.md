# Web UI

Last verified: 2026-02-11

## Purpose
Provides a browser-based dashboard for monitoring and managing Rustagent V2 work graphs. Connects to the daemon HTTP API and WebSocket for real-time updates on agents, tasks, decisions, and sessions.

## Contracts
- **Exposes**: SPA served from `web/dist/` (production build). 8 views: Dashboard, ProjectList, ProjectDetail, TaskTree, DecisionGraph, AgentMonitor, SessionHistory, GraphSearch
- **Guarantees**: TypeScript types in `types.ts` mirror Rust serde serialization exactly (field names, casing, nullability). WebSocket reconnects automatically with exponential backoff (1s to 30s cap). Stores update reactively via Svelte 5 runes (`$state`). Event feed is capped at 200 items (ring buffer).
- **Expects**: Daemon running on port 7400 (Vite proxies `/api` and `/ws` in dev). All API responses match the types defined in `types.ts`.

## Dependencies
- **Uses**: Daemon HTTP API (`/api/*`), Daemon WebSocket (`/ws`), Cytoscape.js (graph visualization), cytoscape-dagre (layout)
- **Used by**: End users via browser. Production build embedded in Rust binary via `rust-embed` behind `bundle-ui` feature flag
- **Boundary**: Does NOT import Rust code directly. All communication is via HTTP/WebSocket. No server-side rendering.

## Key Decisions
- Svelte 5 runes over React: Simpler reactivity model, smaller bundle, native TypeScript support
- Hash-based router over SvelteKit: SPA served from static files, no SSR needed
- Singleton ApiClient: All stores share one instance via `api/index.ts`
- Cytoscape.js for decision graph: Supports dagre layout, node/edge styling by type, interactive selection
- Bun as package manager: Faster installs, compatible lockfile

## Invariants
- `types.ts` enum values must match Rust `#[serde(rename_all)]` output (e.g., `dependson` not `depends_on`, `leadsto` not `leads_to`)
- WebSocket events use discriminated union on `type` field matching Rust `#[serde(tag = "type", rename_all = "snake_case")]`
- Stores never throw; errors are captured in `error` state fields
- Tree building uses "contains" edges for parent-child, "dependson" edges for dependencies

## Key Files
- `src/types.ts` - All TypeScript types mirroring Rust API (GraphNode, GraphEdge, WsEvent, etc.)
- `src/api/client.ts` - ApiClient class wrapping fetch() for all daemon endpoints
- `src/api/websocket.ts` - WsConnection with auto-reconnect and event dispatching
- `src/router.svelte.ts` - Hash-based SPA router with parameter extraction
- `src/stores/` - 4 reactive stores: projects, graph, agents, search
- `src/lib/tree.ts` - GoalTree to nested TreeNode transformation
- `src/lib/decision-graph.ts` - GraphNode/Edge to Cytoscape ElementDefinition conversion
- `src/App.svelte` - App shell: sidebar + routed views + WebSocket lifecycle

## Development
```bash
cd web && bun install        # Install dependencies
cd web && bun run dev        # Dev server with Vite proxy to daemon:7400
cd web && bun run build      # Production build to dist/
cd web && bun run test       # Run vitest tests
```

## Routes
| Hash Path | View | Params |
|-----------|------|--------|
| `/` | Dashboard | - |
| `/projects` | ProjectList | - |
| `/projects/:projectId` | ProjectDetail | projectId |
| `/goals/:goalId/tasks` | TaskTree | goalId |
| `/goals/:goalId/decisions` | DecisionGraph | goalId |
| `/goals/:goalId/agents` | AgentMonitor | goalId |
| `/goals/:goalId/sessions` | SessionHistory | goalId |
| `/search` | GraphSearch | - |
