<script lang="ts">
  /**
   * TaskTree view component.
   * Displays a hierarchical tree of goals, tasks, and subtasks with:
   * - Three-panel layout (tree | detail | ready tasks)
   * - Expandable/collapsible nodes
   * - Status and dependency visualization
   * - Task statistics (progress bar)
   */

  import { getCurrentRoute } from '../router.svelte';
  import { graphState, loadGoalTree, loadReadyTasks, selectNode, clearSelection } from '../stores/graph.svelte';
  import { apiClient } from '../api';
  import { buildTree, countTaskStats, type TreeNode } from '../lib/tree';
  import type { GraphNode } from '../types';
  import TreeNodeRow from '../components/TreeNodeRow.svelte';
  import GraphNodeCard from '../components/GraphNodeCard.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';

  let nextTask: GraphNode | null = $state(null);
  let nodeExpandedState: Record<string, boolean> = $state({});

  // Get goalId from route params
  const currentRoute = $derived(getCurrentRoute());
  const goalId = $derived(currentRoute.params.goalId || '');

  // Load goal tree and ready tasks when goalId changes
  $effect(() => {
    if (goalId) {
      // Trigger async loads
      loadGoalTree(goalId);
      loadReadyTasks(goalId);
      // Load next task directly via API
      loadNextTask();
    }
  });

  async function loadNextTask(): Promise<void> {
    if (!goalId) return;
    try {
      nextTask = await apiClient.getNextTask(goalId);
    } catch (error) {
      // Silently fail on next task (optional feature)
      console.error('Failed to load next task:', error);
    }
  }

  // Build the tree structure from flat goal tree
  const treeRoot = $derived(
    graphState.goalTree ? buildTree(graphState.goalTree) : null
  );

  // Compute task statistics
  const taskStats = $derived(treeRoot ? countTaskStats(treeRoot) : null);

  // Calculate progress percentage
  const progressPercent = $derived(
    taskStats && taskStats.total > 0 ? (taskStats.completed / taskStats.total) * 100 : 0
  );

  // Update node expanded state in tree
  function toggleNodeExpanded(nodeId: string): void {
    if (nodeExpandedState[nodeId] === undefined) {
      nodeExpandedState[nodeId] = false; // Start with expanded=true from buildTree
    } else {
      nodeExpandedState[nodeId] = !nodeExpandedState[nodeId];
    }
    // Trigger reactivity by reassigning the object
    nodeExpandedState = nodeExpandedState;
  }

  /**
   * Apply expanded state to tree recursively.
   */
  function applyExpandedState(node: TreeNode): TreeNode {
    return {
      ...node,
      expanded: nodeExpandedState[node.node.id] !== false, // Default to true
      children: node.children.map((child) => applyExpandedState(child)),
    };
  }

  // Apply expanded state to rendered tree
  const displayTree = $derived(treeRoot ? applyExpandedState(treeRoot) : null);
</script>

<div class="task-tree-container">
  {#if graphState.error}
    <ErrorMessage message={graphState.error} />
  {:else if graphState.loading}
    <LoadingSpinner />
  {:else if !displayTree}
    <div class="empty-state">
      <p>No task tree available for this goal.</p>
    </div>
  {:else}
    <div class="layout">
      <!-- Left Panel: Task Tree -->
      <div class="left-panel">
        <div class="tree-header">
          <h2>Task Hierarchy</h2>
          {#if taskStats}
            <div class="progress-section">
              <div class="progress-bar">
                <div class="progress-fill" style="width: {progressPercent}%"></div>
              </div>
              <div class="progress-text">
                {taskStats.completed} / {taskStats.total} completed
              </div>
            </div>
          {/if}
        </div>

        <div class="tree-content">
          <TreeNodeRow
            treeNode={displayTree}
            depth={0}
            selectedNodeId={graphState.selectedNodeId}
            onSelect={selectNode}
            onToggleExpanded={toggleNodeExpanded}
          />
        </div>
      </div>

      <!-- Right Panel: Node Detail -->
      <div class="right-panel">
        {#if graphState.selectedNodeDetail}
          <div class="detail-header">
            <h3>Node Details</h3>
            <button class="close-btn" onclick={() => clearSelection()}>✕</button>
          </div>
          <div class="detail-content">
            <GraphNodeCard node={graphState.selectedNodeDetail.node} />

            {#if graphState.selectedNodeDetail.outgoing_edges}
              {@const dependsOnEdges = graphState.selectedNodeDetail.outgoing_edges.filter(([edge]) => edge.edge_type === 'dependson')}
              {#if dependsOnEdges.length > 0}
                <div class="dependencies-section">
                  <h4>Dependencies</h4>
                  <ul class="dependency-list">
                    {#each dependsOnEdges as [edge, depNode] (edge.id)}
                      <li>
                        <button class="link-btn" onclick={() => selectNode(depNode.id)}>
                          {depNode.title} ({depNode.id})
                        </button>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
            {/if}

            {#if graphState.selectedNodeDetail.incoming_edges && graphState.selectedNodeDetail.incoming_edges.length > 0}
              <div class="edges-section">
                <h4>Incoming Edges</h4>
                <ul class="edges-list">
                  {#each graphState.selectedNodeDetail.incoming_edges as [edge, sourceNode] (edge.id)}
                    <li>{edge.edge_type}: {sourceNode.title} ({sourceNode.id})</li>
                  {/each}
                </ul>
              </div>
            {/if}

            {#if graphState.selectedNodeDetail.outgoing_edges && graphState.selectedNodeDetail.outgoing_edges.length > 0}
              <div class="edges-section">
                <h4>Outgoing Edges</h4>
                <ul class="edges-list">
                  {#each graphState.selectedNodeDetail.outgoing_edges as [edge, targetNode] (edge.id)}
                    <li>{edge.edge_type}: {targetNode.title} ({targetNode.id})</li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>
        {:else}
          <div class="empty-detail">
            <p>Select a node to view details</p>
          </div>
        {/if}
      </div>
    </div>

    <!-- Bottom Panel: Ready Tasks -->
    {#if graphState.readyTasks.length > 0}
      <div class="bottom-panel">
        <h3>Ready Tasks</h3>
        <div class="ready-tasks-list">
          {#each graphState.readyTasks as task (task.id)}
            <button
              class="ready-task-item"
              class:next-task={nextTask && task.id === nextTask.id}
              onclick={() => selectNode(task.id)}
            >
              <div class="task-title">{task.title}</div>
              {#if nextTask && task.id === nextTask.id}
                <span class="next-badge">Next</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .task-tree-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: white;
    overflow: hidden;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6b7280;
  }

  .layout {
    display: flex;
    flex: 1;
    gap: 1rem;
    padding: 1rem;
    overflow: hidden;
  }

  /* Left Panel */
  .left-panel {
    flex: 0 0 50%;
    display: flex;
    flex-direction: column;
    border: 1px solid #e5e7eb;
    border-radius: 0.5rem;
    background: white;
    overflow: hidden;
  }

  .tree-header {
    padding: 1rem;
    border-bottom: 1px solid #e5e7eb;
    background: #f9fafb;
  }

  .tree-header h2 {
    margin: 0 0 1rem 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: #111827;
  }

  .progress-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .progress-bar {
    height: 0.5rem;
    background: #e5e7eb;
    border-radius: 9999px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #10b981;
    transition: width 0.3s ease;
  }

  .progress-text {
    font-size: 0.75rem;
    color: #6b7280;
  }

  .tree-content {
    flex: 1;
    overflow-y: auto;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
  }

  /* Right Panel */
  .right-panel {
    flex: 0 0 40%;
    display: flex;
    flex-direction: column;
    border: 1px solid #e5e7eb;
    border-radius: 0.5rem;
    background: white;
    overflow: hidden;
  }

  .detail-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    border-bottom: 1px solid #e5e7eb;
    background: #f9fafb;
  }

  .detail-header h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: #111827;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: #6b7280;
    padding: 0;
    width: 2rem;
    height: 2rem;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close-btn:hover {
    color: #111827;
  }

  .detail-content {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }

  .empty-detail {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #6b7280;
  }

  .dependencies-section,
  .edges-section {
    margin-top: 1.5rem;
    padding-top: 1rem;
    border-top: 1px solid #e5e7eb;
  }

  .dependencies-section h4,
  .edges-section h4 {
    margin: 0 0 0.75rem 0;
    font-size: 0.875rem;
    font-weight: 600;
    color: #374151;
  }

  .dependency-list,
  .edges-list {
    margin: 0;
    padding-left: 1.5rem;
    list-style: none;
  }

  .dependency-list li,
  .edges-list li {
    font-size: 0.875rem;
    color: #374151;
    margin-bottom: 0.5rem;
  }

  .link-btn {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    text-decoration: underline;
    font-size: 0.875rem;
    padding: 0;
  }

  .link-btn:hover {
    color: #2563eb;
  }

  /* Bottom Panel */
  .bottom-panel {
    padding: 1rem;
    border-top: 1px solid #e5e7eb;
    background: #f9fafb;
    max-height: 150px;
    overflow-y: auto;
  }

  .bottom-panel h3 {
    margin: 0 0 0.75rem 0;
    font-size: 1rem;
    font-weight: 600;
    color: #111827;
  }

  .ready-tasks-list {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .ready-task-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: white;
    border: 1px solid #cbd5e1;
    padding: 0.5rem 0.75rem;
    border-radius: 0.375rem;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    color: #111827;
    transition: all 0.2s ease;
  }

  .ready-task-item:hover {
    background: #f0f9ff;
    border-color: #3b82f6;
  }

  .ready-task-item.next-task {
    background: #fef3c7;
    border-color: #fbbf24;
    font-weight: 600;
  }

  .task-title {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .next-badge {
    background: #fbbf24;
    color: #78350f;
    padding: 0.125rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
  }
</style>
