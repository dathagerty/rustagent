/**
 * Tree transformation utilities for hierarchical task visualization.
 * Transforms flat GoalTree (nodes + edges) into nested TreeNode structures.
 */

import type { GraphNode, GraphEdge, GoalTree, NodeStatus } from '../types';

/**
 * TreeNode represents a node in the hierarchical tree structure.
 * Includes nesting relationships, dependencies, and expanded state.
 */
export type TreeNode = {
  node: GraphNode;
  children: Array<TreeNode>;
  dependencies: Array<GraphNode>;
  expanded: boolean;
};

/**
 * TaskStats represents counts of tasks by status.
 */
export type TaskStats = {
  total: number;
  completed: number;
  inProgress: number;
  blocked: number;
  ready: number;
};

/**
 * Compares two node IDs for natural sorting (handles dotted hierarchy).
 * Sorts ra-xxxx.1 before ra-xxxx.2.
 */
function naturalSortNodeIds(id1: string, id2: string): number {
  const parts1 = id1.split('.');
  const parts2 = id2.split('.');

  // Compare each part as a number if possible, then as string.
  for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
    const p1 = parts1[i] ?? '';
    const p2 = parts2[i] ?? '';

    const num1 = parseInt(p1, 10);
    const num2 = parseInt(p2, 10);

    if (!isNaN(num1) && !isNaN(num2)) {
      if (num1 !== num2) return num1 - num2;
    } else {
      if (p1 !== p2) return p1.localeCompare(p2);
    }
  }

  return 0;
}

/**
 * Builds a nested TreeNode from flat GoalTree (nodes + edges).
 *
 * Algorithm:
 * 1. Find root: the node with no incoming "Contains" edges (should be the goal).
 * 2. Build children: for each node, find nodes connected via outgoing "Contains" edges.
 * 3. Sort children by ID (natural sort).
 * 4. Find dependencies: nodes connected via "dependson" edges.
 * 5. Recursively build nested structure.
 *
 * @param goalTree - Flat nodes and edges from API
 * @returns Root TreeNode with nested children
 * @throws Error if no root node found (nodes list is empty)
 */
export function buildTree(goalTree: GoalTree): TreeNode {
  const { nodes, edges } = goalTree;

  if (nodes.length === 0) {
    throw new Error('Cannot build tree: no nodes provided');
  }

  // Find the root node: no incoming "Contains" edges.
  const incomingContainsEdges = new Set<string>();
  edges.forEach((edge) => {
    if (edge.edge_type === 'contains') {
      incomingContainsEdges.add(edge.to_node);
    }
  });

  const rootNode = nodes.find((n) => !incomingContainsEdges.has(n.id));
  if (!rootNode) {
    // If no root found with the standard approach, use the first node.
    return buildTreeNode(nodes[0]!, nodes, edges);
  }

  return buildTreeNode(rootNode, nodes, edges);
}

/**
 * Recursively builds a TreeNode for a given node.
 */
function buildTreeNode(node: GraphNode, allNodes: Array<GraphNode>, allEdges: Array<GraphEdge>): TreeNode {
  // Find all children (nodes connected via outgoing "Contains" edges).
  const childIds = allEdges
    .filter((e) => e.edge_type === 'contains' && e.from_node === node.id)
    .map((e) => e.to_node);

  const childNodes = allNodes.filter((n) => childIds.includes(n.id));

  // Sort children by ID (natural sort).
  childNodes.sort((a, b) => naturalSortNodeIds(a.id, b.id));

  // Find dependencies (nodes connected via outgoing "dependson" edges TO this node).
  // Note: dependson edge is from_node (depends on) to to_node (is depended on).
  // So we want edges where to_node === current node, to find what depends on us.
  // But the task description says "dependencies: nodes this one depends on",
  // which means edges where from_node === current node.
  const dependencyIds = allEdges
    .filter((e) => e.edge_type === 'dependson' && e.from_node === node.id)
    .map((e) => e.to_node);

  const dependencies = allNodes.filter((n) => dependencyIds.includes(n.id));

  // Sort dependencies by ID for consistency
  dependencies.sort((a, b) => naturalSortNodeIds(a.id, b.id));

  // Recursively build children.
  const children = childNodes.map((childNode) => buildTreeNode(childNode, allNodes, allEdges));

  return {
    node,
    children,
    dependencies,
    expanded: true,
  };
}

/**
 * Flattens a tree into a depth-first list for rendering.
 * Useful for keyboard navigation and linear iteration.
 *
 * @param root - Root TreeNode
 * @returns Array of TreeNodes in depth-first order
 */
export function flattenTree(root: TreeNode): Array<TreeNode> {
  const result: Array<TreeNode> = [root];

  function traverse(node: TreeNode) {
    node.children.forEach((child) => {
      result.push(child);
      traverse(child);
    });
  }

  traverse(root);
  return result;
}

/**
 * Recursively counts task nodes by status.
 * Used for progress reporting and statistics.
 *
 * @param root - Root TreeNode
 * @returns TaskStats with counts for each status
 */
export function countTaskStats(root: TreeNode): TaskStats {
  let total = 0;
  let completed = 0;
  let inProgress = 0;
  let blocked = 0;
  let ready = 0;

  function traverse(node: TreeNode) {
    // Count only task-type nodes.
    if (node.node.node_type === 'task') {
      total += 1;

      switch (node.node.status) {
        case 'completed':
          completed += 1;
          break;
        case 'in_progress':
          inProgress += 1;
          break;
        case 'blocked':
          blocked += 1;
          break;
        case 'ready':
          ready += 1;
          break;
      }
    }

    node.children.forEach((child) => traverse(child));
  }

  traverse(root);

  return { total, completed, inProgress, blocked, ready };
}
