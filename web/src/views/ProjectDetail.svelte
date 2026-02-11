<script lang="ts">
  /**
   * ProjectDetail view component.
   * Displays project information, goals, and active decisions.
   * Shows goal cards with task completion and navigation to related views.
   */

  import { apiClient } from '../api';
  import { getCurrentRoute } from '../router.svelte';
  import { navigate } from '../router.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';
  import StatusBadge from '../components/StatusBadge.svelte';
  import PriorityBadge from '../components/PriorityBadge.svelte';
  import type { ProjectResponse, GraphNode, GoalTree } from '../types';

  type GoalWithTasks = {
    goal: GraphNode;
    completedTasks: number;
    totalTasks: number;
  };

  type ProjectDetailData = {
    project: ProjectResponse | null;
    goals: Array<GoalWithTasks>;
    decisions: Array<GraphNode>;
  };

  let currentRoute = $derived(getCurrentRoute());
  let projectId = $derived(currentRoute.params.projectId);

  let projectData: ProjectDetailData = $state({
    project: null,
    goals: [],
    decisions: [],
  });

  let loading = $state(true);
  let error = $state<string | null>(null);

  /**
   * Load project details, goals, and decisions on mount or when projectId changes.
   */
  $effect(async () => {
    if (!projectId) {
      error = 'No project ID provided';
      return;
    }

    try {
      loading = true;
      error = null;

      // Load project
      const project = await apiClient.getProject(projectId);

      // Load goals
      const goals = await apiClient.listGoals(projectId);

      // For each goal, load its tree to count tasks
      const goalsWithTasks: Array<GoalWithTasks> = [];
      for (const goal of goals) {
        try {
          const tree = await apiClient.getGoalTree(goal.id);
          const tasks = tree.nodes.filter((n) => n.node_type === 'task');
          const completed = tasks.filter((t) => t.status === 'completed').length;
          goalsWithTasks.push({
            goal,
            completedTasks: completed,
            totalTasks: tasks.length,
          });
        } catch {
          // If tree fetch fails, show goal with 0 tasks
          goalsWithTasks.push({
            goal,
            completedTasks: 0,
            totalTasks: 0,
          });
        }
      }

      // Load decisions
      const decisions = await apiClient.listDecisions(projectId);

      projectData = {
        project,
        goals: goalsWithTasks,
        decisions,
      };
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unknown error';
    } finally {
      loading = false;
    }
  });

  /**
   * Format a date string into human-readable format.
   */
  function formatDate(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      return date.toLocaleDateString('en-US', {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      });
    } catch {
      return dateStr;
    }
  }

  /**
   * Calculate task completion percentage.
   */
  function getTaskPercentage(completed: number, total: number): number {
    if (total === 0) return 0;
    return Math.round((completed / total) * 100);
  }

  /**
   * Handle action link clicks.
   */
  function handleTasksClick(goalId: string): void {
    navigate(`/goals/${goalId}/tasks`);
  }

  function handleDecisionsClick(goalId: string): void {
    navigate(`/goals/${goalId}/decisions`);
  }

  function handleAgentsClick(goalId: string): void {
    navigate(`/goals/${goalId}/agents`);
  }

  function handleSessionsClick(goalId: string): void {
    navigate(`/goals/${goalId}/sessions`);
  }

  function handleDecisionClick(decisionId: string): void {
    navigate(`/goals/${decisionId}/decisions`);
  }
</script>

<div class="project-detail-container">
  {#if loading}
    <div class="loading-state">
      <LoadingSpinner />
      <p>Loading project details...</p>
    </div>
  {:else if error}
    <ErrorMessage message={error} />
  {:else if !projectData.project}
    <ErrorMessage message="Project not found" />
  {:else}
    <div class="project-detail-wrapper">
      <!-- Project Header -->
      <div class="project-header">
        <h1>{projectData.project.name}</h1>
        <div class="project-info">
          <div class="info-item">
            <span class="label">Path:</span>
            <code class="path-code">{projectData.project.path}</code>
          </div>
          <div class="info-item">
            <span class="label">Registered:</span>
            <span class="value">{formatDate(projectData.project.registered_at)}</span>
          </div>
        </div>
      </div>

      <!-- Goals Section -->
      <div class="section">
        <h2>Goals</h2>
        {#if projectData.goals.length === 0}
          <p class="empty-state">No goals in this project.</p>
        {:else}
          <div class="goals-grid">
            {#each projectData.goals as item (item.goal.id)}
              <div class="goal-card">
                <div class="card-header">
                  <h3 class="goal-title">{item.goal.title}</h3>
                  <div class="badges">
                    <StatusBadge status={item.goal.status} />
                    <PriorityBadge priority={item.goal.priority} />
                  </div>
                </div>

                {#if item.goal.description}
                  <p class="goal-description">{item.goal.description}</p>
                {/if}

                <!-- Task Progress -->
                <div class="task-progress">
                  <div class="progress-label">
                    <span class="label">Tasks:</span>
                    <span class="count">{item.completedTasks}/{item.totalTasks}</span>
                  </div>
                  <div class="progress-bar">
                    <div
                      class="progress-fill"
                      style="width: {getTaskPercentage(item.completedTasks, item.totalTasks)}%"
                    ></div>
                  </div>
                  <div class="percentage">
                    {getTaskPercentage(item.completedTasks, item.totalTasks)}%
                  </div>
                </div>

                <!-- Action Links -->
                <div class="actions">
                  <button class="action-button" onclick={() => handleTasksClick(item.goal.id)}>
                    Tasks
                  </button>
                  <button class="action-button" onclick={() => handleDecisionsClick(item.goal.id)}>
                    Decisions
                  </button>
                  <button class="action-button" onclick={() => handleAgentsClick(item.goal.id)}>
                    Agents
                  </button>
                  <button class="action-button" onclick={() => handleSessionsClick(item.goal.id)}>
                    Sessions
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Active Decisions Section -->
      {#if projectData.decisions.length > 0}
        <div class="section">
          <h2>Active Decisions</h2>
          <div class="decisions-list">
            {#each projectData.decisions as decision (decision.id)}
              <div class="decision-item">
                <div class="decision-header">
                  <button
                    class="decision-title-link"
                    onclick={() => handleDecisionClick(decision.id)}
                  >
                    {decision.title}
                  </button>
                  <StatusBadge status={decision.status} />
                </div>
                {#if decision.description}
                  <p class="decision-description">{decision.description}</p>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .project-detail-container {
    padding: 2rem;
    max-width: 1200px;
    margin: 0 auto;
  }

  .loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 4rem 2rem;
    text-align: center;
    color: #9ca3af;
  }

  .project-detail-wrapper {
    width: 100%;
  }

  .project-header {
    margin-bottom: 2rem;
    padding-bottom: 1.5rem;
    border-bottom: 1px solid #404055;
  }

  .project-header h1 {
    margin: 0 0 1rem 0;
    font-size: 2rem;
    font-weight: 700;
    color: #ffffff;
  }

  .project-info {
    display: flex;
    gap: 2rem;
    flex-wrap: wrap;
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .info-item .label {
    font-weight: 600;
    color: #d1d5db;
  }

  .info-item .value {
    color: #9ca3af;
  }

  .path-code {
    background-color: #1a1a2e;
    color: #10b981;
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 0.85rem;
  }

  .section {
    margin-bottom: 2rem;
  }

  .section h2 {
    margin: 0 0 1.5rem 0;
    font-size: 1.5rem;
    font-weight: 600;
    color: #ffffff;
  }

  .empty-state {
    color: #6b7280;
    font-size: 0.875rem;
    margin: 1rem 0 0 0;
  }

  .goals-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
    gap: 1.5rem;
  }

  .goal-card {
    background-color: #242444;
    border: 1px solid #404055;
    border-radius: 0.5rem;
    padding: 1.5rem;
    color: #e0e0e0;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }

  .goal-title {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: #ffffff;
    flex: 1;
  }

  .badges {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .goal-description {
    color: #9ca3af;
    font-size: 0.875rem;
    margin: 0;
  }

  .task-progress {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .progress-label {
    display: flex;
    justify-content: space-between;
    font-size: 0.875rem;
    color: #d1d5db;
  }

  .progress-label .label {
    font-weight: 500;
  }

  .progress-label .count {
    color: #9ca3af;
  }

  .progress-bar {
    height: 8px;
    background-color: #1a1a2e;
    border-radius: 4px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background-color: #3b82f6;
    border-radius: 4px;
    transition: width 0.3s ease;
  }

  .percentage {
    text-align: right;
    font-size: 0.75rem;
    color: #6b7280;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .action-button {
    background-color: #3b82f6;
    color: #ffffff;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.2s ease;
  }

  .action-button:hover {
    background-color: #2563eb;
  }

  .action-button:active {
    background-color: #1d4ed8;
  }

  .decisions-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .decision-item {
    background-color: #242444;
    border: 1px solid #404055;
    border-radius: 0.5rem;
    padding: 1rem;
    color: #e0e0e0;
  }

  .decision-header {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 0.5rem;
  }

  .decision-title-link {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.95rem;
    font-weight: 500;
    padding: 0;
    text-decoration: none;
    flex: 1;
  }

  .decision-title-link:hover {
    color: #60a5fa;
    text-decoration: underline;
  }

  .decision-description {
    color: #9ca3af;
    font-size: 0.875rem;
    margin: 0;
  }
</style>
