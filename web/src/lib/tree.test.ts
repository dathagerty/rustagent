/**
 * Tests for tree transformation utilities.
 * Verifies hierarchical tree building, flattening, and task statistics.
 */

import { describe, it, expect } from 'vitest';
import { buildTree, flattenTree, countTaskStats } from './tree';
import type { GraphNode, GraphEdge, GoalTree } from '../types';

/**
 * Create test fixtures: a goal with 2 tasks (one completed, one in_progress),
 * and a subtask under the first task. Includes Contains edges for hierarchy
 * and a dependson edge for dependencies.
 */
function createTestFixture(): GoalTree {
  const nodes: Array<GraphNode> = [
    {
      id: 'ra-1234',
      project_id: 'proj-1',
      node_type: 'goal',
      title: 'Main Goal',
      description: 'Complete the main objective',
      status: 'active',
      priority: 'high',
      assigned_to: null,
      created_by: 'user-1',
      labels: [],
      created_at: '2026-02-01T00:00:00Z',
      started_at: '2026-02-02T00:00:00Z',
      completed_at: null,
      blocked_reason: null,
      metadata: {},
    },
    {
      id: 'ra-1234.1',
      project_id: 'proj-1',
      node_type: 'task',
      title: 'Task 1 (Completed)',
      description: 'First task that is completed',
      status: 'completed',
      priority: 'high',
      assigned_to: 'agent-1',
      created_by: 'user-1',
      labels: [],
      created_at: '2026-02-01T01:00:00Z',
      started_at: '2026-02-02T01:00:00Z',
      completed_at: '2026-02-05T01:00:00Z',
      blocked_reason: null,
      metadata: {},
    },
    {
      id: 'ra-1234.2',
      project_id: 'proj-1',
      node_type: 'task',
      title: 'Task 2 (In Progress)',
      description: 'Second task that is in progress',
      status: 'in_progress',
      priority: 'medium',
      assigned_to: 'agent-2',
      created_by: 'user-1',
      labels: [],
      created_at: '2026-02-03T00:00:00Z',
      started_at: '2026-02-04T00:00:00Z',
      completed_at: null,
      blocked_reason: null,
      metadata: {},
    },
    {
      id: 'ra-1234.1.1',
      project_id: 'proj-1',
      node_type: 'task',
      title: 'Subtask of Task 1',
      description: 'A subtask under the first task',
      status: 'completed',
      priority: 'low',
      assigned_to: null,
      created_by: 'user-1',
      labels: [],
      created_at: '2026-02-01T02:00:00Z',
      started_at: null,
      completed_at: '2026-02-05T02:00:00Z',
      blocked_reason: null,
      metadata: {},
    },
    {
      id: 'ra-1234.3',
      project_id: 'proj-1',
      node_type: 'task',
      title: 'Task 3 (Ready)',
      description: 'Third task ready to start',
      status: 'ready',
      priority: null,
      assigned_to: null,
      created_by: 'user-1',
      labels: [],
      created_at: '2026-02-06T00:00:00Z',
      started_at: null,
      completed_at: null,
      blocked_reason: null,
      metadata: {},
    },
  ];

  const edges: Array<GraphEdge> = [
    // Hierarchy: goal contains tasks
    {
      id: 'e-1',
      edge_type: 'contains',
      from_node: 'ra-1234',
      to_node: 'ra-1234.1',
      label: null,
      created_at: '2026-02-01T00:00:00Z',
    },
    {
      id: 'e-2',
      edge_type: 'contains',
      from_node: 'ra-1234',
      to_node: 'ra-1234.2',
      label: null,
      created_at: '2026-02-01T00:00:00Z',
    },
    {
      id: 'e-3',
      edge_type: 'contains',
      from_node: 'ra-1234',
      to_node: 'ra-1234.3',
      label: null,
      created_at: '2026-02-01T00:00:00Z',
    },
    // Hierarchy: task 1 contains subtask
    {
      id: 'e-4',
      edge_type: 'contains',
      from_node: 'ra-1234.1',
      to_node: 'ra-1234.1.1',
      label: null,
      created_at: '2026-02-01T02:00:00Z',
    },
    // Dependency: task 2 depends on task 1
    {
      id: 'e-5',
      edge_type: 'dependson',
      from_node: 'ra-1234.2',
      to_node: 'ra-1234.1',
      label: null,
      created_at: '2026-02-03T00:00:00Z',
    },
    // Dependency: task 3 depends on task 2
    {
      id: 'e-6',
      edge_type: 'dependson',
      from_node: 'ra-1234.3',
      to_node: 'ra-1234.2',
      label: null,
      created_at: '2026-02-06T00:00:00Z',
    },
  ];

  return { nodes, edges };
}

describe('tree transformation utilities', () => {
  describe('buildTree', () => {
    it('builds correct nesting with root as goal and tasks as children', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);

      // Root should be the goal
      expect(tree.node.id).toBe('ra-1234');
      expect(tree.node.node_type).toBe('goal');

      // Should have 3 direct children (tasks)
      expect(tree.children.length).toBe(3);

      // Children should be the tasks, sorted by ID
      expect(tree.children[0]!.node.id).toBe('ra-1234.1');
      expect(tree.children[1]!.node.id).toBe('ra-1234.2');
      expect(tree.children[2]!.node.id).toBe('ra-1234.3');
    });

    it('nests subtasks under their parent tasks', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);

      // Task 1 should have 1 child (the subtask)
      const task1 = tree.children[0];
      expect(task1!.children.length).toBe(1);
      expect(task1!.children[0]!.node.id).toBe('ra-1234.1.1');
      expect(task1!.children[0]!.node.title).toBe('Subtask of Task 1');
    });

    it('correctly identifies dependencies from dependson edges', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);

      // Task 2 should have Task 1 as a dependency
      const task2 = tree.children[1];
      expect(task2!.dependencies.length).toBe(1);
      expect(task2!.dependencies[0]!.id).toBe('ra-1234.1');

      // Task 3 should have Task 2 as a dependency
      const task3 = tree.children[2];
      expect(task3!.dependencies.length).toBe(1);
      expect(task3!.dependencies[0]!.id).toBe('ra-1234.2');

      // Task 1 should have no dependencies
      const task1 = tree.children[0];
      expect(task1!.dependencies.length).toBe(0);
    });

    it('sorts children by ID (natural sort)', () => {
      const nodes: Array<GraphNode> = [
        {
          id: 'ra-1000',
          project_id: 'proj-1',
          node_type: 'goal',
          title: 'Goal',
          description: 'Goal',
          status: 'active',
          priority: null,
          assigned_to: null,
          created_by: null,
          labels: [],
          created_at: '2026-02-01T00:00:00Z',
          started_at: null,
          completed_at: null,
          blocked_reason: null,
          metadata: {},
        },
        {
          id: 'ra-1000.10',
          project_id: 'proj-1',
          node_type: 'task',
          title: 'Task 10',
          description: '',
          status: 'pending',
          priority: null,
          assigned_to: null,
          created_by: null,
          labels: [],
          created_at: '2026-02-01T00:00:00Z',
          started_at: null,
          completed_at: null,
          blocked_reason: null,
          metadata: {},
        },
        {
          id: 'ra-1000.2',
          project_id: 'proj-1',
          node_type: 'task',
          title: 'Task 2',
          description: '',
          status: 'pending',
          priority: null,
          assigned_to: null,
          created_by: null,
          labels: [],
          created_at: '2026-02-01T00:00:00Z',
          started_at: null,
          completed_at: null,
          blocked_reason: null,
          metadata: {},
        },
        {
          id: 'ra-1000.1',
          project_id: 'proj-1',
          node_type: 'task',
          title: 'Task 1',
          description: '',
          status: 'pending',
          priority: null,
          assigned_to: null,
          created_by: null,
          labels: [],
          created_at: '2026-02-01T00:00:00Z',
          started_at: null,
          completed_at: null,
          blocked_reason: null,
          metadata: {},
        },
      ];

      const edges: Array<GraphEdge> = [
        {
          id: 'e-1',
          edge_type: 'contains',
          from_node: 'ra-1000',
          to_node: 'ra-1000.10',
          label: null,
          created_at: '2026-02-01T00:00:00Z',
        },
        {
          id: 'e-2',
          edge_type: 'contains',
          from_node: 'ra-1000',
          to_node: 'ra-1000.2',
          label: null,
          created_at: '2026-02-01T00:00:00Z',
        },
        {
          id: 'e-3',
          edge_type: 'contains',
          from_node: 'ra-1000',
          to_node: 'ra-1000.1',
          label: null,
          created_at: '2026-02-01T00:00:00Z',
        },
      ];

      const tree = buildTree({ nodes, edges });

      // Children should be sorted: .1, .2, .10
      expect(tree.children[0]!.node.id).toBe('ra-1000.1');
      expect(tree.children[1]!.node.id).toBe('ra-1000.2');
      expect(tree.children[2]!.node.id).toBe('ra-1000.10');
    });

    it('initializes all TreeNodes with expanded = true', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);

      expect(tree.expanded).toBe(true);
      tree.children.forEach((child) => {
        expect(child.expanded).toBe(true);
      });
    });

    it('throws error when nodes array is empty', () => {
      const emptyGoalTree: GoalTree = { nodes: [], edges: [] };
      expect(() => buildTree(emptyGoalTree)).toThrow('Cannot build tree: no nodes provided');
    });
  });

  describe('flattenTree', () => {
    it('returns nodes in depth-first order', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);
      const flattened = flattenTree(tree);

      // Depth-first order: root, then task1 and its subtask, then task2, then task3
      expect(flattened.length).toBeGreaterThan(0);
      expect(flattened[0]!.node.id).toBe('ra-1234'); // root first
      expect(flattened[1]!.node.id).toBe('ra-1234.1'); // task 1
      expect(flattened[2]!.node.id).toBe('ra-1234.1.1'); // subtask (nested under task 1)
      expect(flattened[3]!.node.id).toBe('ra-1234.2'); // task 2
      expect(flattened[4]!.node.id).toBe('ra-1234.3'); // task 3
    });

    it('includes all nodes from the tree', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);
      const flattened = flattenTree(tree);

      // 1 goal + 3 tasks + 1 subtask = 5 nodes
      expect(flattened.length).toBe(5);
    });
  });

  describe('countTaskStats', () => {
    it('counts tasks by status correctly', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);
      const stats = countTaskStats(tree);

      // Fixture has: 1 completed (task 1 and subtask), 1 in_progress (task 2), 1 ready (task 3)
      // Total should count all 4 tasks (not the goal)
      expect(stats.total).toBe(4);
      expect(stats.completed).toBe(2); // task 1 and subtask
      expect(stats.inProgress).toBe(1); // task 2
      expect(stats.ready).toBe(1); // task 3
      expect(stats.blocked).toBe(0);
    });

    it('does not count non-task nodes', () => {
      const fixture = createTestFixture();
      const tree = buildTree(fixture);
      const stats = countTaskStats(tree);

      // Should count only task-type nodes, not the goal node
      expect(stats.total).toBe(4); // not 5 (excludes the goal)
    });

    it('handles empty tree with only goal node', () => {
      const nodes: Array<GraphNode> = [
        {
          id: 'ra-5000',
          project_id: 'proj-1',
          node_type: 'goal',
          title: 'Lonely Goal',
          description: 'A goal with no tasks',
          status: 'active',
          priority: null,
          assigned_to: null,
          created_by: null,
          labels: [],
          created_at: '2026-02-01T00:00:00Z',
          started_at: null,
          completed_at: null,
          blocked_reason: null,
          metadata: {},
        },
      ];

      const tree = buildTree({ nodes, edges: [] });
      const stats = countTaskStats(tree);

      expect(stats.total).toBe(0);
      expect(stats.completed).toBe(0);
      expect(stats.inProgress).toBe(0);
      expect(stats.blocked).toBe(0);
      expect(stats.ready).toBe(0);
    });
  });
});
