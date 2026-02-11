/**
 * Reactive store for work graph state using Svelte 5 runes.
 * Manages nodes, edges, goal trees, tasks, and decisions.
 */

import { createApiClient } from '../api/client';
import type {
  GoalTree,
  GraphNode,
  DecisionHistory,
  NodeWithEdges,
  NodeStatus,
  WsEvent,
} from '../types';

/**
 * Graph state object.
 * Managed as Svelte 5 $state for reactivity.
 */
export const graphState = $state<{
  goalTree: GoalTree | null;
  selectedGoalId: string | null;
  selectedNodeId: string | null;
  selectedNodeDetail: NodeWithEdges | null;
  tasks: Array<GraphNode>;
  readyTasks: Array<GraphNode>;
  decisions: Array<GraphNode>;
  decisionHistory: DecisionHistory | null;
  loading: boolean;
  error: string | null;
}>({
  goalTree: null,
  selectedGoalId: null,
  selectedNodeId: null,
  selectedNodeDetail: null,
  tasks: [],
  readyTasks: [],
  decisions: [],
  decisionHistory: null,
  loading: false,
  error: null,
});

/**
 * Get goal nodes from the current goal tree.
 */
export function getGoalNodes(): Array<GraphNode> {
  if (!graphState.goalTree) {
    return [];
  }
  return graphState.goalTree.nodes.filter((n) => n.node_type === 'goal');
}

/**
 * Get tasks grouped by status.
 */
export function getTasksByStatus(): Record<NodeStatus, Array<GraphNode>> {
  const result: Record<NodeStatus, Array<GraphNode>> = {} as Record<
    NodeStatus,
    Array<GraphNode>
  >;

  for (const task of graphState.tasks) {
    if (!result[task.status]) {
      result[task.status] = [];
    }
    result[task.status].push(task);
  }

  return result;
}

const apiClient = createApiClient();

/**
 * Load all goals for a project.
 */
export async function loadGoals(projectId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    // Store goals in a temporary location; they're not directly shown in graphState
    // but are used to populate the goal tree
    await apiClient.listGoals(projectId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Select a goal by ID.
 */
export function selectGoal(id: string): void {
  graphState.selectedGoalId = id;
}

/**
 * Load the full goal tree for a goal.
 */
export async function loadGoalTree(goalId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.goalTree = await apiClient.getGoalTree(goalId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Load all tasks for a goal.
 */
export async function loadTasks(goalId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.tasks = await apiClient.listTasks(goalId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Load ready tasks for a goal.
 */
export async function loadReadyTasks(goalId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.readyTasks = await apiClient.listReadyTasks(goalId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Load all decisions for a project.
 */
export async function loadDecisions(projectId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.decisions = await apiClient.listDecisions(projectId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Load decision history for a project.
 */
export async function loadDecisionHistory(projectId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.decisionHistory = await apiClient.getDecisionHistory(projectId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Load detailed node information.
 */
export async function loadNodeDetail(nodeId: string): Promise<void> {
  graphState.loading = true;
  graphState.error = null;
  try {
    graphState.selectedNodeDetail = await apiClient.getNode(nodeId);
  } catch (error) {
    graphState.error = error instanceof Error ? error.message : 'Unknown error';
  } finally {
    graphState.loading = false;
  }
}

/**
 * Select a node by ID and load its details.
 */
export async function selectNode(id: string): Promise<void> {
  graphState.selectedNodeId = id;
  await loadNodeDetail(id);
}

/**
 * Clear node selection.
 */
export function clearSelection(): void {
  graphState.selectedNodeId = null;
  graphState.selectedNodeDetail = null;
}

/**
 * Handle WebSocket events for graph updates.
 */
export function handleWsEvent(event: WsEvent): void {
  if (event.type === 'node_created') {
    // Add node to goal tree if same goal
    if (graphState.goalTree && event.parent_id) {
      const existingNode = graphState.goalTree.nodes.find((n) => n.id === event.id);
      if (!existingNode) {
        graphState.goalTree.nodes.push({
          id: event.id,
          project_id: event.project_id,
          node_type: event.node_type,
          title: event.title,
          description: event.description,
          status: event.status,
          priority: event.priority,
          assigned_to: event.assigned_to,
          created_by: event.created_by,
          labels: event.labels,
          created_at: event.created_at,
          started_at: event.started_at,
          completed_at: event.completed_at,
          blocked_reason: event.blocked_reason,
          metadata: event.metadata,
        });
      }
    }
  } else if (event.type === 'node_status_changed') {
    // Update node status in goal tree and tasks
    if (graphState.goalTree) {
      const treeNode = graphState.goalTree.nodes.find((n) => n.id === event.node_id);
      if (treeNode) {
        treeNode.status = event.new_status as NodeStatus;
      }
    }

    const taskNode = graphState.tasks.find((n) => n.id === event.node_id);
    if (taskNode) {
      taskNode.status = event.new_status as NodeStatus;
    }
  } else if (event.type === 'edge_created') {
    // Add edge to goal tree
    if (graphState.goalTree) {
      const existingEdge = graphState.goalTree.edges.find((e) => e.id === event.id);
      if (!existingEdge) {
        graphState.goalTree.edges.push({
          id: event.id,
          edge_type: event.edge_type,
          from_node: event.from_node,
          to_node: event.to_node,
          label: event.label,
          created_at: event.created_at,
        });
      }
    }
  }
}
