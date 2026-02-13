<script lang="ts">
  /**
   * Main app shell component.
   * Provides layout with sidebar navigation and routed main content area.
   * Manages WebSocket connection and dispatches events to stores.
   */

  import { getCurrentRoute, initRouter, routerState } from './router.svelte';
  import { createWsConnection } from './api/websocket';
  import { handleWsEvent as handleGraphWsEvent } from './stores/graph.svelte';
  import { handleWsEvent as handleAgentsWsEvent } from './stores/agents.svelte';
  import type { WsEvent } from './types';
  import Sidebar from './components/Sidebar.svelte';
  import Dashboard from './views/Dashboard.svelte';
  import ProjectList from './views/ProjectList.svelte';
  import ProjectDetail from './views/ProjectDetail.svelte';
  import TaskTree from './views/TaskTree.svelte';
  import DecisionGraph from './views/DecisionGraph.svelte';
  import AgentMonitor from './views/AgentMonitor.svelte';
  import SessionHistory from './views/SessionHistory.svelte';
  import GraphSearch from './views/GraphSearch.svelte';
  import Placeholder from './views/Placeholder.svelte';

  // Initialize hash-based router (must run in component context)
  initRouter();

  let wsConnection: ReturnType<typeof createWsConnection> | null = null;

  // Initialize WebSocket connection on mount
  $effect(() => {
    if (!wsConnection) {
      wsConnection = createWsConnection();
      wsConnection.connect();

      // Register event handler that dispatches to all stores
      const handleEvent = (event: WsEvent) => {
        handleGraphWsEvent(event);
        handleAgentsWsEvent(event);
      };

      wsConnection.onEvent(handleEvent);

      // Cleanup on unmount
      return () => {
        if (wsConnection) {
          wsConnection.disconnect();
          wsConnection = null;
        }
      };
    }
  });

  // Get current route
  const currentRoute = $derived(getCurrentRoute());
</script>

<div class="app-container">
  <Sidebar />

  <main class="main-content">
    {#if currentRoute.name === 'dashboard'}
      <Dashboard />
    {:else if currentRoute.name === 'project-list'}
      <ProjectList />
    {:else if currentRoute.name === 'project-detail'}
      <ProjectDetail />
    {:else if currentRoute.name === 'task-tree'}
      <TaskTree />
    {:else if currentRoute.name === 'decision-graph'}
      <DecisionGraph />
    {:else if currentRoute.name === 'agent-monitor'}
      <AgentMonitor />
    {:else if currentRoute.name === 'session-history'}
      <SessionHistory />
    {:else if currentRoute.name === 'graph-search'}
      <GraphSearch />
    {:else}
      <Placeholder name="Not Found" />
    {/if}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  .app-container {
    display: flex;
    width: 100vw;
    height: 100vh;
  }

  .main-content {
    flex: 1;
    background: #1a1a2e;
    overflow-y: auto;
    padding: 0;
  }
</style>
