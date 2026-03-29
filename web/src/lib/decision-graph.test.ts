/**
 * Unit tests for decision-graph transformations.
 * Tests the pure functions: decisionsToElements, filterNowMode, and decisionStylesheet.
 */

import { describe, it, expect } from 'vitest';
import { decisionsToElements, filterNowMode, decisionStylesheet } from './decision-graph';
import type { GraphNode, GraphEdge, NodeType, NodeStatus, EdgeType } from '../types';

/**
 * Create a test GraphNode.
 */
function createNode(
  id: string,
  nodeType: string,
  status: string,
  title: string = 'Test Node'
): GraphNode {
  return {
    id,
    project_id: 'proj-1',
    node_type: nodeType as NodeType,
    title,
    description: 'Test description',
    status: status as NodeStatus,
    priority: 'medium',
    assigned_to: null,
    created_by: null,
    labels: [],
    created_at: '2026-02-11T00:00:00Z',
    started_at: null,
    completed_at: null,
    blocked_reason: null,
    metadata: {},
  };
}

/**
 * Create a test GraphEdge.
 */
function createEdge(
  id: string,
  fromNode: string,
  toNode: string,
  edgeType: string,
  label: string | null = null
): GraphEdge {
  return {
    id,
    edge_type: edgeType as EdgeType,
    from_node: fromNode,
    to_node: toNode,
    label,
    created_at: '2026-02-11T00:00:00Z',
  };
}

describe('decision-graph transformations', () => {
  describe('decisionsToElements', () => {
    it('should convert nodes to Cytoscape element format', () => {
      const nodes: Array<GraphNode> = [createNode('n1', 'decision', 'active', 'Decide X')];
      const edges: Array<GraphEdge> = [];

      const elements = decisionsToElements(nodes, edges);

      const nodeElement = elements.find((el) => el.data?.id === 'n1');
      expect(nodeElement).toBeDefined();
      expect(nodeElement?.data).toMatchObject({
        id: 'n1',
        label: 'Decide X',
        type: 'decision',
        status: 'active',
      });
    });

    it('should convert edges to Cytoscape element format', () => {
      const nodes: Array<GraphNode> = [];
      const edges: Array<GraphEdge> = [
        createEdge('e1', 'n1', 'n2', 'chosen', 'Better approach'),
      ];

      const elements = decisionsToElements(nodes, edges);

      const edgeElement = elements.find((el) => el.data?.id === 'e1');
      expect(edgeElement).toBeDefined();
      expect(edgeElement?.data).toMatchObject({
        id: 'e1',
        source: 'n1',
        target: 'n2',
        label: 'Better approach',
        edgeType: 'chosen',
      });
    });

    it('should use edge_type as label when label is null', () => {
      const edges: Array<GraphEdge> = [
        createEdge('e1', 'n1', 'n2', 'leadsto', null),
      ];

      const elements = decisionsToElements([], edges);

      const edgeElement = elements.find((el) => el.data?.id === 'e1');
      expect(edgeElement?.data?.label).toBe('leadsto');
    });

    it('should return empty array for empty input', () => {
      const elements = decisionsToElements([], []);
      expect(elements).toEqual([]);
    });

    it('should include all metadata fields in node data', () => {
      const node: GraphNode = {
        id: 'n1',
        project_id: 'proj-1',
        node_type: 'decision',
        title: 'Decide X',
        description: 'Test description',
        status: 'active',
        priority: 'high',
        assigned_to: 'agent1',
        created_by: 'user1',
        labels: ['important'],
        created_at: '2026-02-11T00:00:00Z',
        started_at: '2026-02-11T01:00:00Z',
        completed_at: null,
        blocked_reason: null,
        metadata: { key: 'value' },
      };

      const elements = decisionsToElements([node], []);

      const nodeElement = elements.find((el) => el.data?.id === 'n1');
      expect(nodeElement?.data).toMatchObject({
        priority: 'high',
        assigned_to: 'agent1',
        metadata: { key: 'value' },
      });
    });
  });

  describe('filterNowMode', () => {
    it('should include active decisions', () => {
      const nodes: Array<GraphNode> = [
        createNode('d1', 'decision', 'active'),
        createNode('d2', 'decision', 'decided'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      expect(filtered.map((n) => n.id)).toContain('d1');
      expect(filtered.map((n) => n.id)).toContain('d2');
    });

    it('should exclude rejected decisions', () => {
      const nodes: Array<GraphNode> = [
        createNode('d1', 'decision', 'active'),
        createNode('d2', 'decision', 'rejected'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      expect(filtered.map((n) => n.id)).toContain('d1');
      expect(filtered.map((n) => n.id)).not.toContain('d2');
    });

    it('should include chosen options only', () => {
      const nodes: Array<GraphNode> = [
        createNode('o1', 'option', 'chosen'),
        createNode('o2', 'option', 'rejected'),
        createNode('o3', 'option', 'abandoned'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      expect(filtered.map((n) => n.id)).toContain('o1');
      expect(filtered.map((n) => n.id)).not.toContain('o2');
      expect(filtered.map((n) => n.id)).not.toContain('o3');
    });

    it('should include active/completed outcomes', () => {
      const nodes: Array<GraphNode> = [
        createNode('oc1', 'outcome', 'active'),
        createNode('oc2', 'outcome', 'completed'),
        createNode('oc3', 'outcome', 'abandoned'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      expect(filtered.map((n) => n.id)).toContain('oc1');
      expect(filtered.map((n) => n.id)).toContain('oc2');
      expect(filtered.map((n) => n.id)).not.toContain('oc3');
    });

    it('should exclude other node types', () => {
      const nodes: Array<GraphNode> = [
        createNode('g1', 'goal', 'active'),
        createNode('t1', 'task', 'completed'),
        createNode('obs1', 'observation', 'active'),
        createNode('d1', 'decision', 'active'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      expect(filtered.map((n) => n.id)).not.toContain('g1');
      expect(filtered.map((n) => n.id)).not.toContain('t1');
      expect(filtered.map((n) => n.id)).not.toContain('obs1');
      expect(filtered.map((n) => n.id)).toContain('d1');
    });

    it('should prune edges where either endpoint is filtered out', () => {
      const nodes: Array<GraphNode> = [
        createNode('d1', 'decision', 'active'),
        createNode('o1', 'option', 'chosen'),
        createNode('o2', 'option', 'rejected'),
      ];
      const edges: Array<GraphEdge> = [
        createEdge('e1', 'd1', 'o1', 'chosen'), // both endpoints included
        createEdge('e2', 'd1', 'o2', 'chosen'), // o2 is filtered out
        createEdge('e3', 'o2', 'o1', 'leadsto'), // o2 is filtered out
      ];

      const { edges: filtered } = filterNowMode(nodes, edges);

      expect(filtered.map((e) => e.id)).toContain('e1');
      expect(filtered.map((e) => e.id)).not.toContain('e2');
      expect(filtered.map((e) => e.id)).not.toContain('e3');
    });

    it('should handle empty input arrays', () => {
      const { nodes, edges } = filterNowMode([], []);

      expect(nodes).toEqual([]);
      expect(edges).toEqual([]);
    });

    it('should include revisit nodes in history mode only', () => {
      const nodes: Array<GraphNode> = [
        createNode('rv1', 'revisit', 'active'),
        createNode('d1', 'decision', 'active'),
      ];

      const { nodes: filtered } = filterNowMode(nodes, []);

      // Revisit nodes are not included in Now mode
      expect(filtered.map((n) => n.id)).not.toContain('rv1');
      expect(filtered.map((n) => n.id)).toContain('d1');
    });
  });

  describe('decisionStylesheet', () => {
    it('should return a non-empty stylesheet array', () => {
      const stylesheet = decisionStylesheet();

      expect(Array.isArray(stylesheet)).toBe(true);
      expect(stylesheet.length).toBeGreaterThan(0);
    });

    it('should include node styling rules', () => {
      const stylesheet = decisionStylesheet();

      const nodeSelectors = stylesheet.filter((rule) =>
        typeof rule.selector === 'string' && rule.selector.includes('node')
      );
      expect(nodeSelectors.length).toBeGreaterThan(0);
    });

    it('should include edge styling rules', () => {
      const stylesheet = decisionStylesheet();

      const edgeSelectors = stylesheet.filter((rule) =>
        typeof rule.selector === 'string' && rule.selector.includes('edge')
      );
      expect(edgeSelectors.length).toBeGreaterThan(0);
    });

    it('should include decision node styling', () => {
      const stylesheet = decisionStylesheet();

      const decisionRule = stylesheet.find(
        (rule) => typeof rule.selector === 'string' && rule.selector.includes('decision')
      );
      expect(decisionRule).toBeDefined();
      if (decisionRule && 'style' in decisionRule && decisionRule.style) {
        expect((decisionRule.style as Record<string, unknown>)['shape']).toBe('diamond');
      }
    });

    it('should include option node styling', () => {
      const stylesheet = decisionStylesheet();

      const optionRule = stylesheet.find(
        (rule) => typeof rule.selector === 'string' && rule.selector.includes('option')
      );
      expect(optionRule).toBeDefined();
      if (optionRule && 'style' in optionRule && optionRule.style) {
        expect((optionRule.style as Record<string, unknown>)['shape']).toBe('hexagon');
      }
    });

    it('should include outcome node styling', () => {
      const stylesheet = decisionStylesheet();

      const outcomeRule = stylesheet.find(
        (rule) => typeof rule.selector === 'string' && rule.selector.includes('outcome')
      );
      expect(outcomeRule).toBeDefined();
      if (outcomeRule && 'style' in outcomeRule && outcomeRule.style) {
        expect((outcomeRule.style as Record<string, unknown>)['shape']).toBe('ellipse');
      }
    });

    it('should include revisit node styling', () => {
      const stylesheet = decisionStylesheet();

      const revisitRule = stylesheet.find(
        (rule) => typeof rule.selector === 'string' && rule.selector.includes('revisit')
      );
      expect(revisitRule).toBeDefined();
      if (revisitRule && 'style' in revisitRule && revisitRule.style) {
        expect((revisitRule.style as Record<string, unknown>)['shape']).toBe('triangle');
      }
    });

    it('should style chosen options as green', () => {
      const stylesheet = decisionStylesheet();

      const chosenRule = stylesheet.find(
        (rule) =>
          typeof rule.selector === 'string' &&
          rule.selector.includes('option') &&
          rule.selector.includes('chosen')
      );
      expect(chosenRule).toBeDefined();
      if (chosenRule && 'style' in chosenRule && chosenRule.style) {
        expect((chosenRule.style as Record<string, unknown>)['background-color']).toBe('#32CD32');
      }
    });

    it('should style rejected options as gray with dashed border', () => {
      const stylesheet = decisionStylesheet();

      const rejectedRule = stylesheet.find(
        (rule) =>
          typeof rule.selector === 'string' &&
          rule.selector.includes('option') &&
          rule.selector.includes('rejected')
      );
      expect(rejectedRule).toBeDefined();
      if (rejectedRule && 'style' in rejectedRule && rejectedRule.style) {
        expect((rejectedRule.style as Record<string, unknown>)['background-color']).toBe('#666');
        expect((rejectedRule.style as Record<string, unknown>)['border-style']).toBe('dashed');
      }
    });

    it('should style decision nodes as gold', () => {
      const stylesheet = decisionStylesheet();

      const decisionRule = stylesheet.find(
        (rule) => typeof rule.selector === 'string' && rule.selector === 'node[type = "decision"]'
      );
      expect(decisionRule).toBeDefined();
      if (decisionRule && 'style' in decisionRule && decisionRule.style) {
        expect((decisionRule.style as Record<string, unknown>)['background-color']).toBe('#FFD700');
      }
    });

    it('should style chosen edges as green and thick', () => {
      const stylesheet = decisionStylesheet();

      const chosenEdgeRule = stylesheet.find(
        (rule) =>
          typeof rule.selector === 'string' &&
          rule.selector.includes('edge') &&
          rule.selector.includes('chosen')
      );
      expect(chosenEdgeRule).toBeDefined();
      if (chosenEdgeRule && 'style' in chosenEdgeRule && chosenEdgeRule.style) {
        expect((chosenEdgeRule.style as Record<string, unknown>)['line-color']).toBe('#32CD32');
        expect((chosenEdgeRule.style as Record<string, unknown>)['width']).toBe(3);
      }
    });

    it('should style rejected edges as dashed gray', () => {
      const stylesheet = decisionStylesheet();

      const rejectedEdgeRule = stylesheet.find(
        (rule) =>
          typeof rule.selector === 'string' &&
          rule.selector.includes('edge') &&
          rule.selector.includes('rejected')
      );
      expect(rejectedEdgeRule).toBeDefined();
      if (rejectedEdgeRule && 'style' in rejectedEdgeRule && rejectedEdgeRule.style) {
        expect((rejectedEdgeRule.style as Record<string, unknown>)['line-style']).toBe('dashed');
      }
    });
  });

  describe('integration tests', () => {
    it('should handle a realistic decision graph scenario', () => {
      // Create a realistic scenario with decisions, options, and outcomes
      const nodes: Array<GraphNode> = [
        createNode('d1', 'decision', 'decided', 'Choose database'),
        createNode('o1', 'option', 'chosen', 'PostgreSQL'),
        createNode('o2', 'option', 'rejected', 'MongoDB'),
        createNode('oc1', 'outcome', 'completed', 'DB configured'),
      ];

      const edges: Array<GraphEdge> = [
        createEdge('e1', 'd1', 'o1', 'chosen', 'Relational requirement'),
        createEdge('e2', 'd1', 'o2', 'rejected', 'Incompatible schema'),
        createEdge('e3', 'o1', 'oc1', 'leadsto', ''),
      ];

      // Test Now mode filtering
      const { nodes: nowNodes, edges: nowEdges } = filterNowMode(nodes, edges);

      // Should include decided decision and chosen option
      expect(nowNodes.map((n) => n.id)).toContain('d1');
      expect(nowNodes.map((n) => n.id)).toContain('o1');
      expect(nowNodes.map((n) => n.id)).toContain('oc1');

      // Should exclude rejected option
      expect(nowNodes.map((n) => n.id)).not.toContain('o2');

      // Edges should only connect included nodes
      expect(nowEdges.length).toBeLessThan(edges.length);

      // Test element conversion
      const elements = decisionsToElements(nodes, edges);
      expect(elements.length).toBeGreaterThan(0);

      // Verify styling
      const stylesheet = decisionStylesheet();
      expect(stylesheet.length).toBeGreaterThan(0);
    });

    it('should handle history mode with all nodes', () => {
      // In history mode, we use the full unfiltered data
      const nodes: Array<GraphNode> = [
        createNode('d1', 'decision', 'decided', 'Choose database'),
        createNode('o1', 'option', 'chosen', 'PostgreSQL'),
        createNode('o2', 'option', 'rejected', 'MongoDB'),
        createNode('o3', 'option', 'abandoned', 'SQLite'),
      ];

      const edges: Array<GraphEdge> = [
        createEdge('e1', 'd1', 'o1', 'chosen'),
        createEdge('e2', 'd1', 'o2', 'rejected'),
        createEdge('e3', 'd1', 'o3', 'rejected'),
      ];

      // Without filtering (history mode), all should be included
      const elements = decisionsToElements(nodes, edges);

      // Should have all nodes + all edges
      const nodeCount = elements.filter((el) => !el.data?.source).length;
      expect(nodeCount).toBe(4);

      const edgeCount = elements.filter((el) => el.data?.source).length;
      expect(edgeCount).toBe(3);
    });
  });
});
