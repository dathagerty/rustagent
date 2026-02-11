<script lang="ts">
  /**
   * Dashboard view component.
   * Shows an at-a-glance overview of all active goals across projects,
   * running agents, and recent activity.
   *
   * Fetches data from daemon API via stores.
   */

  import type { ActiveAgent } from '../types';
  import { loadProjects, projectsState } from '../stores/projects.svelte';
  import { apiClient } from '../api';
  import { navigate } from '../router.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';
  import StatusBadge from '../components/StatusBadge.svelte';
  import PriorityBadge from '../components/PriorityBadge.svelte';
  import type { GraphNode } from '../types';

  type DashboardData = {
    projectGoals: Array<{ projectName: string; goalCount: number; projectId: string }>;
    activeGoals: Array<{ goal: GraphNode; projectName: string }>;
    activeAgentsCount: number;
  };

  let dashboardData: DashboardData = $state({
    projectGoals: [],
    activeGoals: [],
    activeAgentsCount: 0,
  });

  let loading = $state(true);
  let error = $state<string | null>(null);

  /**
   * Load dashboard data on mount.
   * Read reactive dependencies synchronously, then call async function.
   */
  $effect(() => {
    // Read reactive deps synchronously so Svelte tracks them
    const projects = projectsState.projects;
    loadDashboardData(projects);
  });

  /**
   * Async function to load all dashboard data.
   * Parallelizes goal fetches per-project, then parallelizes agent fetches.
   */
  async function loadDashboardData(projects: typeof projectsState.projects): Promise<void> {
    try {
      loading = true;
      error = null;

      // Load all projects first
      await loadProjects();

      // Fetch goals for all projects in parallel using Promise.allSettled
      const goalResults = await Promise.allSettled(
        projects.map((project) => apiClient.listGoals(project.id))
      );

      const projectGoals: DashboardData['projectGoals'] = [];
      const allActiveGoals: DashboardData['activeGoals'] = [];
      const agentFetchPromises: Array<Promise<Array<ActiveAgent>>> = [];

      // Process goal results
      goalResults.forEach((result, idx) => {
        const project = projects[idx];
        if (result.status === 'fulfilled') {
          const goals = result.value;
          projectGoals.push({
            projectName: project.name,
            goalCount: goals.length,
            projectId: project.id,
          });

          // Collect active goals
          const activeGoalsForProject = goals.filter((g) => g.status === 'active');
          activeGoalsForProject.forEach((goal) => {
            allActiveGoals.push({
              goal,
              projectName: project.name,
            });

            // Queue agent fetch for this goal
            agentFetchPromises.push(
              apiClient.listAgents(goal.id).catch(() => []) // Return empty array on error
            );
          });
        }
      });

      // Fetch all agents in parallel using Promise.allSettled
      const agentResults = await Promise.allSettled(agentFetchPromises);

      let totalActiveAgents = 0;
      agentResults.forEach((result) => {
        if (result.status === 'fulfilled') {
          totalActiveAgents += result.value.length;
        }
      });

      dashboardData = {
        projectGoals,
        activeGoals: allActiveGoals,
        activeAgentsCount: totalActiveAgents,
      };
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unknown error';
    } finally {
      loading = false;
    }
  }

  /**
   * Handle project click - navigate to project detail.
   */
  function handleProjectClick(projectId: string): void {
    navigate(`/projects/${projectId}`);
  }

  /**
   * Handle goal click - navigate to task tree.
   */
  function handleGoalClick(goalId: string): void {
    navigate(`/goals/${goalId}/tasks`);
  }
</script>

<div class="dashboard">
  {#if loading}
    <div class="loading-state">
      <LoadingSpinner />
      <p>Loading dashboard...</p>
    </div>
  {:else if error}
    <ErrorMessage message={error} />
  {:else}
    <div class="dashboard-grid">
      <!-- Projects summary card -->
      <div class="card">
        <h2>Projects</h2>
        <p class="card-stat">{dashboardData.projectGoals.length}</p>
        <div class="project-list">
          {#each dashboardData.projectGoals as item (item.projectId)}
            <div class="project-row">
              <button
                class="project-link"
                onclick={() => handleProjectClick(item.projectId)}
              >
                {item.projectName}
              </button>
              <span class="goal-count">{item.goalCount} goal{item.goalCount !== 1 ? 's' : ''}</span>
            </div>
          {/each}
        </div>
      </div>

      <!-- Active goals card -->
      <div class="card">
        <h2>Active Goals</h2>
        {#if dashboardData.activeGoals.length > 0}
          <div class="goals-list">
            {#each dashboardData.activeGoals as item (item.goal.id)}
              <div class="goal-row">
                <div class="goal-header">
                  <button
                    class="goal-title-link"
                    onclick={() => handleGoalClick(item.goal.id)}
                  >
                    {item.goal.title}
                  </button>
                  <StatusBadge status={item.goal.status} />
                  <PriorityBadge priority={item.goal.priority} />
                </div>
                <div class="goal-meta">
                  <span class="project-name">{item.projectName}</span>
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <p class="empty-state">No active goals</p>
        {/if}
      </div>

      <!-- Agents card -->
      <div class="card">
        <h2>Agents</h2>
        <p class="card-stat">{dashboardData.activeAgentsCount}</p>
        <p class="card-desc">Active agents currently running</p>
      </div>
    </div>
  {/if}
</div>

<style>
  .dashboard {
    padding: 2rem;
    max-width: 1400px;
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

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
    gap: 1.5rem;
  }

  .card {
    background-color: #242444;
    border: 1px solid #404055;
    border-radius: 0.5rem;
    padding: 1.5rem;
    color: #e0e0e0;
  }

  .card h2 {
    margin: 0 0 1rem 0;
    font-size: 1.125rem;
    font-weight: 600;
    color: #ffffff;
  }

  .card-stat {
    font-size: 2.5rem;
    font-weight: bold;
    color: #3b82f6;
    margin: 0 0 0.5rem 0;
  }

  .card-desc {
    font-size: 0.875rem;
    color: #9ca3af;
    margin: 0;
  }

  .project-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .project-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem;
    background-color: #1a1a2e;
    border-radius: 0.375rem;
  }

  .project-link {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.95rem;
    font-weight: 500;
    padding: 0;
    text-decoration: none;
  }

  .project-link:hover {
    color: #60a5fa;
    text-decoration: underline;
  }

  .goal-count {
    font-size: 0.875rem;
    color: #6b7280;
  }

  .goals-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .goal-row {
    padding: 0.75rem;
    background-color: #1a1a2e;
    border-radius: 0.375rem;
  }

  .goal-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
  }

  .goal-title-link {
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

  .goal-title-link:hover {
    color: #60a5fa;
    text-decoration: underline;
  }

  .goal-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .project-name {
    font-size: 0.8rem;
    color: #9ca3af;
  }

  .empty-state {
    color: #6b7280;
    font-size: 0.875rem;
    margin: 1rem 0 0 0;
  }
</style>
