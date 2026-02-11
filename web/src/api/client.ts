/**
 * Typed HTTP API client for Rustagent V2 daemon.
 * Wraps fetch() calls with proper error handling and type safety.
 */

import type {
  GraphNode,
  GraphEdge,
  ProjectResponse,
  Session,
  ActiveAgent,
  NodeWithEdges,
  GoalTree,
  DecisionHistory,
  ExportResult,
  ImportResult,
  DiffResult,
  CreateProjectRequest,
  CreateGoalRequest,
  UpdateNodeRequest,
  CreateChildRequest,
  CreateEdgeRequest,
  SearchRequest,
  ImportRequest,
} from '../types';

/**
 * API error thrown when a request fails.
 */
export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/**
 * Configurable HTTP API client.
 * Base URL defaults to empty string for same-origin requests (via Vite proxy in dev).
 */
export class ApiClient {
  constructor(private readonly baseUrl: string = '') {}

  /**
   * Generic request method with error handling.
   */
  private async request<T>(
    method: string,
    path: string,
    body?: unknown
  ): Promise<T> {
    const opts: RequestInit = {
      method,
      headers: body ? { 'Content-Type': 'application/json' } : {},
      body: body ? JSON.stringify(body) : undefined,
    };

    const res = await fetch(`${this.baseUrl}${path}`, opts);

    if (!res.ok) {
      const text = await res.text();
      throw new ApiError(res.status, text);
    }

    if (res.status === 204) {
      return undefined as T;
    }

    return res.json();
  }

  // ============ Projects ============

  async listProjects(): Promise<Array<ProjectResponse>> {
    return this.request('GET', '/api/projects');
  }

  async createProject(req: CreateProjectRequest): Promise<ProjectResponse> {
    return this.request('POST', '/api/projects', req);
  }

  async getProject(id: string): Promise<ProjectResponse> {
    return this.request('GET', `/api/projects/${id}`);
  }

  async deleteProject(id: string): Promise<void> {
    return this.request('DELETE', `/api/projects/${id}`);
  }

  // ============ Goals ============

  async listGoals(projectId: string): Promise<Array<GraphNode>> {
    return this.request('GET', `/api/projects/${projectId}/goals`);
  }

  async createGoal(projectId: string, req: CreateGoalRequest): Promise<GraphNode> {
    return this.request('POST', `/api/projects/${projectId}/goals`, req);
  }

  // ============ Nodes ============

  async getNode(id: string): Promise<NodeWithEdges> {
    return this.request('GET', `/api/nodes/${id}`);
  }

  async updateNode(id: string, req: UpdateNodeRequest): Promise<GraphNode> {
    return this.request('PATCH', `/api/nodes/${id}`, req);
  }

  async createChild(parentId: string, req: CreateChildRequest): Promise<GraphNode> {
    return this.request('POST', `/api/nodes/${parentId}/children`, req);
  }

  // ============ Edges ============

  async createEdge(req: CreateEdgeRequest): Promise<GraphEdge> {
    return this.request('POST', '/api/edges', req);
  }

  async deleteEdge(id: string): Promise<void> {
    return this.request('DELETE', `/api/edges/${id}`);
  }

  // ============ Goal Tree and Task Views ============

  async getGoalTree(goalId: string): Promise<GoalTree> {
    return this.request('GET', `/api/goals/${goalId}/tree`);
  }

  async listTasks(goalId: string): Promise<Array<GraphNode>> {
    return this.request('GET', `/api/goals/${goalId}/tasks`);
  }

  async listReadyTasks(goalId: string): Promise<Array<GraphNode>> {
    return this.request('GET', `/api/goals/${goalId}/tasks/ready`);
  }

  async getNextTask(goalId: string): Promise<GraphNode | null> {
    return this.request('GET', `/api/goals/${goalId}/tasks/next`);
  }

  // ============ Decisions ============

  async listDecisions(projectId: string): Promise<Array<GraphNode>> {
    return this.request('GET', `/api/projects/${projectId}/decisions`);
  }

  async getDecisionHistory(projectId: string): Promise<DecisionHistory> {
    return this.request('GET', `/api/projects/${projectId}/decisions/history`);
  }

  async exportDecisions(projectId: string): Promise<Array<string>> {
    return this.request('POST', `/api/projects/${projectId}/decisions/export`);
  }

  // ============ Graph Import/Export ============

  async exportAllGoals(projectId: string): Promise<Array<ExportResult>> {
    return this.request('GET', `/api/projects/${projectId}/graph/export`);
  }

  async exportGoal(goalId: string): Promise<ExportResult> {
    return this.request('GET', `/api/goals/${goalId}/export`);
  }

  async importGraph(projectId: string, req: ImportRequest): Promise<ImportResult> {
    return this.request('POST', `/api/projects/${projectId}/graph/import`, req);
  }

  async diffGraph(projectId: string, req: ImportRequest): Promise<DiffResult> {
    return this.request('POST', `/api/projects/${projectId}/graph/diff`, req);
  }

  // ============ Sessions ============

  async listSessions(goalId: string): Promise<Array<Session>> {
    return this.request('GET', `/api/goals/${goalId}/sessions`);
  }

  async getSession(id: string): Promise<Session> {
    return this.request('GET', `/api/sessions/${id}`);
  }

  // ============ Search ============

  async searchNodes(projectId: string, req: SearchRequest): Promise<Array<GraphNode>> {
    return this.request('POST', `/api/projects/${projectId}/search`, req);
  }

  // ============ Agents ============

  async listAgents(goalId: string): Promise<Array<ActiveAgent>> {
    return this.request('GET', `/api/goals/${goalId}/agents`);
  }

  // ============ Health ============

  async healthCheck(): Promise<{ status: string }> {
    return this.request('GET', '/api/health');
  }
}

/**
 * Singleton-style factory for creating an API client instance.
 */
export function createApiClient(baseUrl?: string): ApiClient {
  return new ApiClient(baseUrl);
}
