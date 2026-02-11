<script lang="ts">
  /**
   * Sidebar navigation component.
   * Provides links to main views and a project selector.
   */

  import { navigate, routerState } from '../router.svelte';
  import { projectsState, getSelectedProject, loadProjects } from '../stores/projects.svelte';

  // Determine if a link is active based on current route
  function isActive(route: string): boolean {
    return routerState.path === route;
  }

  // Track whether we've attempted to load projects to avoid infinite retries on error
  let loadAttempted = false;

  // Load projects on mount
  $effect(() => {
    if (projectsState.projects.length === 0 && !loadAttempted) {
      loadAttempted = true;
      loadProjects();
    }
  });
</script>

<aside class="sidebar">
  <nav class="nav">
    <div class="nav-section">
      <a
        href="/#/"
        class="nav-link"
        class:active={isActive('/')}
        onclick={(e) => {
          e.preventDefault();
          navigate('/');
        }}
      >
        Dashboard
      </a>
      <a
        href="/#/projects"
        class="nav-link"
        class:active={isActive('/projects')}
        onclick={(e) => {
          e.preventDefault();
          navigate('/projects');
        }}
      >
        Projects
      </a>
      <a
        href="/#/search"
        class="nav-link"
        class:active={isActive('/search')}
        onclick={(e) => {
          e.preventDefault();
          navigate('/search');
        }}
      >
        Search
      </a>
    </div>

    {#if projectsState.projects.length > 0}
      <div class="nav-section">
        <div class="section-title">Projects</div>
        <div class="project-list">
          {#each projectsState.projects as project (project.id)}
            {@const selected = getSelectedProject()}
            <button
              class="project-item"
              class:active={selected?.id === project.id}
              onclick={() => navigate(`/projects/${project.id}`)}
            >
              {project.name}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </nav>
</aside>

<style>
  .sidebar {
    width: 240px;
    background: #12122a;
    border-right: 1px solid #2a2a4a;
    height: 100%;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .nav {
    padding: 1rem 0;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .nav-section {
    padding: 0.5rem 0;
  }

  .nav-link {
    display: block;
    padding: 0.75rem 1.5rem;
    color: #a0a0b0;
    text-decoration: none;
    transition: all 0.2s ease;
    border-left: 3px solid transparent;
  }

  .nav-link:hover {
    background: #1a1a3a;
    color: #e0e0e0;
    border-left-color: #6a5acd;
  }

  .nav-link.active {
    background: #1a1a3a;
    color: #e0e0e0;
    border-left-color: #6a5acd;
  }

  .section-title {
    padding: 0.5rem 1.5rem;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    color: #6a6a7a;
    letter-spacing: 0.5px;
  }

  .project-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0 0.5rem;
  }

  .project-item {
    padding: 0.5rem 1rem;
    margin: 0 0.5rem;
    background: transparent;
    border: none;
    color: #a0a0b0;
    text-align: left;
    cursor: pointer;
    transition: all 0.2s ease;
    border-radius: 0.25rem;
    font-size: 0.875rem;
  }

  .project-item:hover {
    background: #1a1a3a;
    color: #e0e0e0;
  }

  .project-item.active {
    background: #2a2a4a;
    color: #e0e0e0;
    font-weight: 600;
  }
</style>
