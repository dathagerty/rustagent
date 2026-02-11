<script lang="ts">
  /**
   * ProjectList view component.
   * Displays all registered projects in a table with navigation to ProjectDetail.
   */

  import { loadProjects, projectsState } from '../stores/projects.svelte';
  import { navigate } from '../router.svelte';
  import { formatDate } from '../lib/date-formatting';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';

  let loading = $state(true);
  let error = $state<string | null>(null);

  /**
   * Load projects on mount.
   * Read reactive dependencies synchronously, then call async function.
   */
  $effect(() => {
    // Synchronously trigger load on mount
    loadProjectsData();
  });

  /**
   * Async function to load projects.
   */
  async function loadProjectsData(): Promise<void> {
    try {
      loading = true;
      error = null;
      await loadProjects();
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unknown error';
    } finally {
      loading = false;
    }
  }

  /**
   * Handle project row click - navigate to ProjectDetail.
   */
  function handleProjectClick(projectId: string): void {
    navigate(`/projects/${projectId}`);
  }
</script>

<div class="project-list-container">
  {#if loading}
    <div class="loading-state">
      <LoadingSpinner />
      <p>Loading projects...</p>
    </div>
  {:else if error}
    <ErrorMessage message={error} />
  {:else if projectsState.projects.length === 0}
    <div class="empty-state">
      <p>No projects registered.</p>
      <p class="instruction">Use `rustagent project add &lt;name&gt; &lt;path&gt;` to register a project.</p>
    </div>
  {:else}
    <div class="project-list-wrapper">
      <h1>Projects</h1>
      <table class="projects-table">
        <thead>
          <tr>
            <th class="name-col">Name</th>
            <th class="path-col">Path</th>
            <th class="date-col">Registered</th>
          </tr>
        </thead>
        <tbody>
          {#each projectsState.projects as project (project.id)}
            <tr class="project-row" onclick={() => handleProjectClick(project.id)}>
              <td class="name-cell">
                <button class="project-name-link" onclick={() => handleProjectClick(project.id)}>
                  {project.name}
                </button>
              </td>
              <td class="path-cell">
                <code class="path-code">{project.path}</code>
              </td>
              <td class="date-cell">
                {formatDate(project.registered_at)}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .project-list-container {
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

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    padding: 4rem 2rem;
    text-align: center;
    color: #9ca3af;
  }

  .empty-state p {
    margin: 0;
  }

  .instruction {
    font-size: 0.875rem;
    color: #6b7280;
  }

  .project-list-wrapper {
    width: 100%;
  }

  .project-list-wrapper h1 {
    margin: 0 0 1.5rem 0;
    font-size: 1.875rem;
    font-weight: 700;
    color: #ffffff;
  }

  .projects-table {
    width: 100%;
    border-collapse: collapse;
    background-color: #242444;
    border: 1px solid #404055;
    border-radius: 0.5rem;
    overflow: hidden;
  }

  .projects-table thead {
    background-color: #1a1a2e;
    border-bottom: 1px solid #404055;
  }

  .projects-table th {
    padding: 1rem;
    text-align: left;
    font-size: 0.875rem;
    font-weight: 600;
    color: #d1d5db;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .name-col {
    width: 30%;
  }

  .path-col {
    width: 50%;
  }

  .date-col {
    width: 20%;
  }

  .project-row {
    border-bottom: 1px solid #404055;
    transition: background-color 0.2s ease;
    cursor: pointer;
  }

  .project-row:hover {
    background-color: #2a2a3e;
  }

  .project-row:last-child {
    border-bottom: none;
  }

  .projects-table td {
    padding: 1rem;
    color: #e0e0e0;
  }

  .name-cell {
    font-weight: 500;
  }

  .project-name-link {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.95rem;
    font-weight: 500;
    padding: 0;
    text-decoration: none;
  }

  .project-name-link:hover {
    color: #60a5fa;
    text-decoration: underline;
  }

  .path-code {
    background-color: #1a1a2e;
    color: #10b981;
    padding: 0.25rem 0.5rem;
    border-radius: 0.25rem;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 0.85rem;
  }

  .date-cell {
    font-size: 0.875rem;
    color: #9ca3af;
  }
</style>
