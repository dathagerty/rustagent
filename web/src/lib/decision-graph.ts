/**
 * Decision graph transformation utilities.
 * Converts API responses to Cytoscape element definitions and manages filtering for Now/History modes.
 */

import type { ElementDefinition } from 'cytoscape';
import type cytoscape from 'cytoscape';
import type { GraphNode, GraphEdge, NodeType, NodeStatus, EdgeType } from '../types';

/**
 * Convert GraphNode and GraphEdge arrays to Cytoscape ElementDefinition format.
 * Nodes include type and status for styling.
 * Edges include edgeType for styling and label from the edge data.
 */
export function decisionsToElements(
  nodes: Array<GraphNode>,
  edges: Array<GraphEdge>,
): Array<ElementDefinition> {
  const nodeElements: Array<ElementDefinition> = nodes.map((node) => ({
    data: {
      id: node.id,
      label: node.title,
      type: node.node_type,
      status: node.status,
      description: node.description,
      priority: node.priority,
      assigned_to: node.assigned_to,
      metadata: node.metadata,
    },
  }));

  const edgeElements: Array<ElementDefinition> = edges.map((edge) => ({
    data: {
      id: edge.id,
      source: edge.from_node,
      target: edge.to_node,
      label: edge.label || edge.edge_type,
      edgeType: edge.edge_type,
    },
  }));

  return [...nodeElements, ...edgeElements];
}

/**
 * Filter nodes and edges for "Now" mode (current active decisions).
 * Includes:
 *   - Decision nodes with status 'active' or 'decided'
 *   - Option nodes with status 'chosen'
 *   - Outcome nodes with status 'active' or 'completed'
 * Excludes: 'rejected', 'abandoned', 'superseded' nodes
 * Prunes edges where either endpoint is filtered out.
 */
export function filterNowMode(
  nodes: Array<GraphNode>,
  edges: Array<GraphEdge>,
): { nodes: Array<GraphNode>; edges: Array<GraphEdge> } {
  // Determine which nodes to include
  const nowModeStatuses: Set<NodeStatus> = new Set([
    'active',
    'decided',
    'chosen',
    'completed',
  ]);

  const filteredNodes = nodes.filter((node) => {
    const isDecision = node.node_type === 'decision';
    const isOption = node.node_type === 'option';
    const isOutcome = node.node_type === 'outcome';

    // Decision: include if active or decided
    if (isDecision) {
      return node.status === 'active' || node.status === 'decided';
    }

    // Option: include only if chosen
    if (isOption) {
      return node.status === 'chosen';
    }

    // Outcome: include if active or completed
    if (isOutcome) {
      return node.status === 'active' || node.status === 'completed';
    }

    // Other node types not included in decision graph
    return false;
  });

  const includedNodeIds = new Set(filteredNodes.map((n) => n.id));

  // Filter edges: both endpoints must exist in filtered nodes
  const filteredEdges = edges.filter(
    (edge) => includedNodeIds.has(edge.from_node) && includedNodeIds.has(edge.to_node),
  );

  return {
    nodes: filteredNodes,
    edges: filteredEdges,
  };
}

/**
 * Generate the Cytoscape stylesheet for decision graph visualization.
 * Styles nodes by type and status, edges by type and relationship.
 */
export function decisionStylesheet(): cytoscape.StylesheetJson {
  return [
    // Base node styling
    {
      selector: 'node',
      style: {
        'content': 'data(label)',
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'font-size': 12,
        'font-weight': 'normal',
        'color': '#000',
        'background-color': '#ccc',
        'border-color': '#333',
        'border-width': 2,
        'padding': '10px',
        'min-zoomed-font-size': 10,
      },
    },

    // Decision nodes: diamond shape, gold background
    {
      selector: 'node[type = "decision"]',
      style: {
        'shape': 'diamond',
        'background-color': '#FFD700',
        'border-color': '#DAA520',
      },
    },

    // Option nodes: hexagon shape
    {
      selector: 'node[type = "option"]',
      style: {
        'shape': 'hexagon',
      },
    },

    // Option chosen: green background
    {
      selector: 'node[type = "option"][status = "chosen"]',
      style: {
        'background-color': '#32CD32',
        'border-color': '#228B22',
      },
    },

    // Option rejected: gray background, dashed border
    {
      selector: 'node[type = "option"][status = "rejected"]',
      style: {
        'background-color': '#666',
        'border-color': '#333',
        'border-style': 'dashed',
      },
    },

    // Option abandoned: muted red, dashed border
    {
      selector: 'node[type = "option"][status = "abandoned"]',
      style: {
        'background-color': '#CD5C5C',
        'border-color': '#8B3A3A',
        'border-style': 'dashed',
      },
    },

    // Outcome nodes: ellipse shape
    {
      selector: 'node[type = "outcome"]',
      style: {
        'shape': 'ellipse',
      },
    },

    // Outcome completed: green
    {
      selector: 'node[type = "outcome"][status = "completed"]',
      style: {
        'background-color': '#32CD32',
        'border-color': '#228B22',
      },
    },

    // Outcome active: blue
    {
      selector: 'node[type = "outcome"][status = "active"]',
      style: {
        'background-color': '#4169E1',
        'border-color': '#00008B',
      },
    },

    // Revisit nodes: triangle, orange
    {
      selector: 'node[type = "revisit"]',
      style: {
        'shape': 'triangle',
        'background-color': '#FFA500',
        'border-color': '#FF8C00',
      },
    },

    // Selected node: highlighted border
    {
      selector: 'node:selected',
      style: {
        'border-width': 3,
        'border-color': '#FF0000',
      },
    },

    // Base edge styling
    {
      selector: 'edge',
      style: {
        'line-color': '#999',
        'target-arrow-color': '#999',
        'target-arrow-shape': 'triangle',
        'curve-style': 'straight',
        'content': 'data(label)',
        'font-size': 11,
        'text-background-color': '#fff',
        'text-background-padding': '3px',
      },
    },

    // Chosen edges: thicker, green
    {
      selector: 'edge[edgeType = "chosen"]',
      style: {
        'line-color': '#32CD32',
        'target-arrow-color': '#32CD32',
        'width': 3,
      },
    },

    // Rejected edges: dashed, gray
    {
      selector: 'edge[edgeType = "rejected"]',
      style: {
        'line-color': '#999',
        'target-arrow-color': '#999',
        'line-style': 'dashed',
        'width': 1,
      },
    },

    // Leads-to edges: normal style
    {
      selector: 'edge[edgeType = "leadsto"]',
      style: {
        'line-color': '#666',
        'target-arrow-color': '#666',
        'width': 2,
      },
    },

    // Depends-on edges: normal style
    {
      selector: 'edge[edgeType = "dependson"]',
      style: {
        'line-color': '#666',
        'target-arrow-color': '#666',
        'width': 2,
      },
    },
  ];
}
