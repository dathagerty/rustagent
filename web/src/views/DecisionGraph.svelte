<script lang="ts">
  /**
   * DecisionGraph view component.
   * Displays an interactive decision graph using Cytoscape.js with Now/History mode toggle.
   * Shows decision nodes (diamonds), option nodes (hexagons), outcomes, and revisit nodes.
   * Supports both active-decisions-only (Now) and full-evolution (History) modes.
   */

  import { getCurrentRoute } from '../router.svelte';
  import { projectsState, getSelectedProject } from '../stores/projects.svelte';
  import {
    graphState,
    loadDecisions,
    loadDecisionHistory,
    selectNode,
    clearSelection,
  } from '../stores/graph.svelte';
  import { apiClient } from '../api';
  import { decisionsToElements, filterNowMode, decisionStylesheet } from '../lib/decision-graph';
  import type { GraphNode, GraphEdge } from '../types';
  import CytoscapeGraph from '../components/CytoscapeGraph.svelte';
  import GraphNodeCard from '../components/GraphNodeCard.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';

  let mode: 'now' | 'history' = $state('now');
  let cytoscapeComponent: InstanceType<typeof CytoscapeGraph> | undefined = $state();

  // Get goalId from route params
  const currentRoute = $derived(getCurrentRoute());
  const goalId = $derived(currentRoute.params.goalId || '');

  // Get projectId from projects store
  const selectedProject = $derived(getSelectedProject());
  const projectId = $derived(selectedProject?.id || '');

  /**
   * Load decision data based on current mode.
   */
  async function loadDecisionData(): Promise<void> {
    if (!projectId) return;

    if (mode === 'now') {
      // Load active decisions
      await loadDecisions(projectId);
    } else {
      // Load full decision history
      await loadDecisionHistory(projectId);
    }
  }

  /**
   * When mode or projectId changes, reload data.
   */
  $effect(() => {
    if (projectId) {
      loadDecisionData();
    }
  });

  /**
   * Handle node click to show detail panel.
   */
  async function handleNodeClick(nodeId: string): Promise<void> {
    selectNode(nodeId);
    try {
      const detail = await apiClient.getNode(nodeId);
      graphState.selectedNodeDetail = detail;
    } catch (error) {
      console.error('Failed to load node detail:', error);
      graphState.error = error instanceof Error ? error.message : 'Failed to load node detail';
    }
  }

  /**
   * Fit the graph to the viewport.
   */
  function fitView(): void {
    cytoscapeComponent?.fitView?.();
  }

  /**
   * Compute elements and styling based on current mode.
   */
  function getDisplayData(): Array<Record<string, unknown>> {
    if (mode === 'now' && graphState.decisions.length > 0) {
      // For Now mode, need to load goal tree to get options/outcomes
      // Use decisions + goalTree edges
      const filteredData = filterNowMode(graphState.decisions, graphState.goalTree?.edges || []);
      return decisionsToElements(filteredData.nodes, filteredData.edges);
    } else if (mode === 'history' && graphState.decisionHistory) {
      // For History mode, use all nodes and edges
      return decisionsToElements(
        graphState.decisionHistory.nodes,
        graphState.decisionHistory.edges
      );
    }
    return [];
  }

  const displayData = $derived(getDisplayData());
  const nodeCount = $derived(
    displayData.filter((el) => el.data && (el.data as Record<string, unknown>).id && !(el.data as Record<string, unknown>).source).length
  );
  const stylesheet = $derived(decisionStylesheet());
</script>

<div class="decision-graph-container">
  {#if graphState.error}
    <ErrorMessage message={graphState.error} />
  {:else if graphState.loading}
    <LoadingSpinner />
  {:else}
    <div class="layout">
      <!-- Top Toolbar -->
      <div class="toolbar">
        <div class="mode-toggle">
          <button
            class="toggle-btn {mode === 'now' ? 'active' : ''}"
            onclick={() => {
              mode = 'now';
            }}
          >
            Now
          </button>
          <button
            class="toggle-btn {mode === 'history' ? 'active' : ''}"
            onclick={() => {
              mode = 'history';
            }}
          >
            History
          </button>
        </div>

        <div class="actions">
          <button class="fit-btn" onclick={fitView}>Fit to view</button>
          <span class="node-count">{nodeCount} nodes</span>
        </div>
      </div>

      <!-- Graph and Detail Panel -->
      <div class="content-area">
        <!-- Graph (center) -->
        <div class="graph-area">
          {#if displayData.length > 0}
            <CytoscapeGraph
              bind:this={cytoscapeComponent}
              elements={displayData}
              {stylesheet}
              onNodeClick={handleNodeClick}
              onBackgroundClick={clearSelection}
            />
          {:else}
            <div class="empty-state">
              <p>No decision data available for this project.</p>
            </div>
          {/if}
        </div>

        <!-- Detail Panel (right) -->
        {#if graphState.selectedNodeDetail}
          <div class="detail-panel">
            <div class="detail-header">
              <h3>Node Details</h3>
              <button class="close-btn" onclick={() => clearSelection()}>✕</button>
            </div>
            <div class="detail-content">
              <GraphNodeCard node={graphState.selectedNodeDetail.node} />

              <!-- Show pros/cons for Option nodes -->
              {#if graphState.selectedNodeDetail.node.node_type === 'option'}
                {#if graphState.selectedNodeDetail.node.metadata}
                  {#if graphState.selectedNodeDetail.node.metadata.pros}
                    <div class="metadata-section">
                      <h4>Pros</h4>
                      <ul class="metadata-list">
                        {#each graphState.selectedNodeDetail.node.metadata.pros.split('\n') as pro}
                          {#if pro.trim()}
                            <li>{pro.trim()}</li>
                          {/if}
                        {/each}
                      </ul>
                    </div>
                  {/if}
                  {#if graphState.selectedNodeDetail.node.metadata.cons}
                    <div class="metadata-section">
                      <h4>Cons</h4>
                      <ul class="metadata-list">
                        {#each graphState.selectedNodeDetail.node.metadata.cons.split('\n') as con}
                          {#if con.trim()}
                            <li>{con.trim()}</li>
                          {/if}
                        {/each}
                      </ul>
                    </div>
                  {/if}
                {/if}
              {/if}

              <!-- Show incoming edges for decision nodes (which option was chosen) -->
              {#if graphState.selectedNodeDetail.node.node_type === 'decision' && graphState.selectedNodeDetail.incoming_edges}
                {@const chosenEdges = graphState.selectedNodeDetail.incoming_edges.filter(
                  ([edge]) => edge.edge_type === 'chosen'
                )}
                {#if chosenEdges.length > 0}
                  <div class="metadata-section">
                    <h4>Chosen Option</h4>
                    <ul class="metadata-list">
                      {#each chosenEdges as [edge, optionNode] (edge.id)}
                        <li>
                          {optionNode.title}
                          {#if edge.label}
                            <span class="reason">({edge.label})</span>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  </div>
                {/if}
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .decision-graph-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: #f5f5f5;
    overflow: hidden;
  }

  .layout {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    background-color: #fff;
    border-bottom: 1px solid #e0e0e0;
    gap: 16px;
  }

  .mode-toggle {
    display: flex;
    gap: 8px;
  }

  .toggle-btn {
    padding: 6px 16px;
    border: 1px solid #ddd;
    background-color: #f5f5f5;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    color: #666;
    transition: all 0.2s;
  }

  .toggle-btn:hover {
    background-color: #eee;
    border-color: #999;
  }

  .toggle-btn.active {
    background-color: #2196f3;
    color: #fff;
    border-color: #1976d2;
  }

  .actions {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .fit-btn {
    padding: 6px 16px;
    border: 1px solid #ddd;
    background-color: #fff;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 500;
    color: #666;
    transition: all 0.2s;
  }

  .fit-btn:hover {
    background-color: #f0f0f0;
    border-color: #999;
  }

  .node-count {
    font-size: 13px;
    color: #999;
  }

  .content-area {
    display: flex;
    flex: 1;
    gap: 1px;
    min-height: 0;
    background-color: #e0e0e0;
  }

  .graph-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    background-color: #f5f5f5;
    position: relative;
    min-width: 0;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #999;
    font-size: 14px;
  }

  .detail-panel {
    width: 350px;
    background-color: #fff;
    display: flex;
    flex-direction: column;
    border-left: 1px solid #e0e0e0;
    overflow: hidden;
  }

  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid #e0e0e0;
  }

  .detail-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #333;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 18px;
    cursor: pointer;
    color: #999;
    padding: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: #333;
  }

  .detail-content {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }

  .metadata-section {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid #e0e0e0;
  }

  .metadata-section h4 {
    margin: 0 0 8px 0;
    font-size: 12px;
    font-weight: 600;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .metadata-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .metadata-list li {
    font-size: 13px;
    color: #555;
    margin-bottom: 6px;
    line-height: 1.4;
  }

  .reason {
    font-size: 12px;
    color: #999;
    margin-left: 4px;
  }
</style>
