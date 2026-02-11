/**
 * TypeScript types matching the Rustagent V2 daemon API.
 * Field names, casing, and nullability match Rust serde serialization exactly.
 */

// Enum types using string literal unions matching Rust serde serialization

/**
 * Node type enum.
 * Rust: #[serde(rename_all = "lowercase")]
 */
export type NodeType = 'goal' | 'task' | 'decision' | 'option' | 'outcome' | 'observation' | 'revisit';

/**
 * Edge type enum.
 * Rust: #[serde(rename_all = "lowercase")]
 * WARNING: DependsOn serializes as "dependson" (not "depends_on")
 * and LeadsTo as "leadsto" (not "leads_to"). This is serde's behavior,
 * not the Display impl.
 */
export type EdgeType = 'contains' | 'dependson' | 'leadsto' | 'chosen' | 'rejected' | 'supersedes' | 'informs';

/**
 * Node status enum.
 * Rust: #[serde(rename_all = "snake_case")]
 */
export type NodeStatus =
  | 'pending'
  | 'active'
  | 'completed'
  | 'cancelled'
  | 'ready'
  | 'claimed'
  | 'in_progress'
  | 'review'
  | 'blocked'
  | 'failed'
  | 'decided'
  | 'superseded'
  | 'abandoned'
  | 'chosen'
  | 'rejected';

/**
 * Priority enum.
 * Rust: #[serde(rename_all = "lowercase")]
 */
export type Priority = 'critical' | 'high' | 'medium' | 'low';

// Core API response types

/**
 * Graph node representing a work item.
 * 15 fields matching the Rust GraphNode struct.
 */
export type GraphNode = {
  id: string;
  project_id: string;
  node_type: NodeType;
  title: string;
  description: string;
  status: NodeStatus;
  priority: Priority | null;
  assigned_to: string | null;
  created_by: string | null;
  labels: Array<string>;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  blocked_reason: string | null;
  metadata: Record<string, string>;
};

/**
 * Graph edge representing a relationship between nodes.
 * 6 fields matching the Rust GraphEdge struct.
 */
export type GraphEdge = {
  id: string;
  edge_type: EdgeType;
  from_node: string;
  to_node: string;
  label: string | null;
  created_at: string;
};

/**
 * Project metadata response.
 */
export type ProjectResponse = {
  id: string;
  name: string;
  path: string;
  registered_at: string;
};

/**
 * Session tracking agent work on a goal.
 */
export type Session = {
  id: string;
  project_id: string;
  goal_id: string;
  started_at: string;
  ended_at: string | null;
  handoff_notes: string | null;
  agent_ids: Array<string>;
  summary: string | null;
};

/**
 * Currently active agent on a task.
 */
export type ActiveAgent = {
  agent_id: string;
  task_id: string;
  task_title: string;
  task_status: NodeStatus;
};

// Composite response types

/**
 * Node with its incoming and outgoing edges.
 */
export type NodeWithEdges = {
  node: GraphNode;
  incoming_edges: Array<[GraphEdge, GraphNode]>;
  outgoing_edges: Array<[GraphEdge, GraphNode]>;
};

/**
 * Goal tree structure: nodes and edges for a goal subtree.
 */
export type GoalTree = {
  nodes: Array<GraphNode>;
  edges: Array<GraphEdge>;
};

/**
 * Decision history: nodes and edges for decisions and choices.
 */
export type DecisionHistory = {
  nodes: Array<GraphNode>;
  edges: Array<GraphEdge>;
};

/**
 * Result of exporting a goal to TOML.
 */
export type ExportResult = {
  goal_id: string;
  toml: string;
};

/**
 * Conflict when importing a graph.
 */
export type ImportConflict = {
  node_id: string;
  field: string;
  db_value: string;
  file_value: string;
};

/**
 * Result of importing a graph.
 */
export type ImportResult = {
  added_nodes: number;
  added_edges: number;
  conflicts: Array<ImportConflict>;
  skipped_edges: Array<string>;
  unchanged: number;
};

/**
 * Diff result comparing a graph to the database.
 */
export type DiffResult = {
  added_nodes: Array<string>;
  changed_nodes: Array<[string, Array<string>]>;
  removed_nodes: Array<string>;
  added_edges: Array<string>;
  removed_edges: Array<string>;
  unchanged_nodes: number;
  unchanged_edges: number;
};

// Request body types

/**
 * Create project request.
 */
export type CreateProjectRequest = {
  name: string;
  path: string;
};


/**
 * Create goal request.
 */
export type CreateGoalRequest = {
  title: string;
  description: string;
  priority?: Priority;
};


/**
 * Update node request.
 */
export type UpdateNodeRequest = {
  status?: string;
  title?: string;
  description?: string;
  blocked_reason?: string;
  metadata?: Record<string, string>;
};


/**
 * Create child node request.
 */
export type CreateChildRequest = {
  node_type: NodeType;
  title: string;
  description: string;
  priority?: Priority;
  metadata?: Record<string, string>;
};


/**
 * Create edge request.
 */
export type CreateEdgeRequest = {
  edge_type: EdgeType;
  from_node: string;
  to_node: string;
  label?: string;
};


/**
 * Search request for nodes.
 */
export type SearchRequest = {
  query: string;
  node_type?: NodeType;
  limit?: number;
};


/**
 * Import graph request.
 */
export type ImportRequest = {
  toml: string;
  strategy?: 'merge' | 'theirs' | 'ours';
};

/**
 * WebSocket event discriminated union.
 * Rust: #[serde(tag = "type", rename_all = "snake_case")]
 * All events have a `type` field for discrimination.
 */
export type WsEvent =
  | {
      type: 'agent_spawned';
      agent_id: string;
      profile: string;
      goal_id: string;
    }
  | {
      type: 'agent_progress';
      agent_id: string;
      turn: number;
      summary: string;
    }
  | {
      type: 'agent_completed';
      agent_id: string;
      outcome_type: string;
      summary: string;
      tokens_used: number | null;
    }
  | {
      type: 'node_created';
      parent_id: string | null;
      id: string;
      project_id: string;
      node_type: NodeType;
      title: string;
      description: string;
      status: NodeStatus;
      priority: Priority | null;
      assigned_to: string | null;
      created_by: string | null;
      labels: Array<string>;
      created_at: string;
      started_at: string | null;
      completed_at: string | null;
      blocked_reason: string | null;
      metadata: Record<string, string>;
    }
  | {
      type: 'node_status_changed';
      node_id: string;
      node_type: string;
      old_status: string;
      new_status: string;
    }
  | {
      type: 'edge_created';
      id: string;
      edge_type: EdgeType;
      from_node: string;
      to_node: string;
      label: string | null;
      created_at: string;
    }
  | {
      type: 'session_ended';
      session_id: string;
      handoff_notes: string | null;
    }
  | {
      type: 'tool_execution';
      agent_id: string;
      tool: string;
      args: unknown;
      result: string;
    }
  | {
      type: 'orchestrator_state_changed';
      goal_id: string;
      state: string;
    };
