<script lang="ts">
  /**
   * GraphSearch view component.
   * Full-text search interface with node type filtering.
   * Uses FTS5 search endpoint to find nodes by title/description.
   */

  import { searchState, executeSearch, setQuery, setNodeTypeFilter, clearSearch } from '../stores/search.svelte';
  import { projectsState, getSelectedProject } from '../stores/projects.svelte';
  import SearchResult from '../components/SearchResult.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';
  import type { NodeType } from '../types';

  let selectedProject = $derived(getSelectedProject());
  const nodeTypeFilters: Array<NodeType | 'all'> = ['all', 'goal', 'task', 'decision', 'option', 'outcome', 'observation', 'revisit'];

  /**
   * Handle search button click or Enter key.
   */
  function handleSearch(): void {
    if (!selectedProject) {
      return;
    }
    executeSearch(selectedProject.id);
  }

  /**
   * Handle node type filter toggle.
   */
  function toggleFilter(type: NodeType | 'all'): void {
    if (type === 'all') {
      setNodeTypeFilter(null);
    } else if (searchState.nodeTypeFilter === type) {
      setNodeTypeFilter(null);
    } else {
      setNodeTypeFilter(type as NodeType);
    }
    // Re-execute search with new filter
    if (selectedProject && searchState.query) {
      executeSearch(selectedProject.id);
    }
  }

  /**
   * Handle Enter key in search input.
   */
  function handleKeyDown(event: KeyboardEvent): void {
    if (event.key === 'Enter') {
      handleSearch();
    }
  }

  /**
   * Get display text for a filter button.
   */
  function getFilterText(type: NodeType | 'all'): string {
    if (type === 'all') {
      return 'All';
    }
    return type.charAt(0).toUpperCase() + type.slice(1);
  }

  /**
   * Check if filter is currently active.
   */
  function isFilterActive(type: NodeType | 'all'): boolean {
    if (type === 'all') {
      return searchState.nodeTypeFilter === null;
    }
    return searchState.nodeTypeFilter === type;
  }

  /**
   * Clear search on mount.
   */
  $effect(() => {
    return () => {
      // Cleanup on unmount
    };
  });
</script>

<div class="graph-search">
  <div class="header">
    <h2>Graph Search</h2>
  </div>

  <div class="content">
    {#if !selectedProject}
      <div class="project-selection-prompt">
        <p>Please select a project first to search its work graph.</p>
      </div>
    {:else}
      <div class="search-bar">
        <input
          type="text"
          class="search-input"
          placeholder="Search nodes by title or description..."
          bind:value={searchState.query}
          onkeydown={handleKeyDown}
        />
        <button class="search-button" onclick={handleSearch} disabled={!selectedProject}>
          Search
        </button>
      </div>

      <div class="filter-bar">
        {#each nodeTypeFilters as filterType (filterType)}
          <button
            class="filter-button"
            class:active={isFilterActive(filterType)}
            onclick={() => toggleFilter(filterType)}
          >
            {getFilterText(filterType)}
          </button>
        {/each}
      </div>

      {#if searchState.error}
        <ErrorMessage message={searchState.error} />
      {/if}

      {#if searchState.loading}
        <LoadingSpinner />
      {:else if searchState.query === ''}
        <div class="empty-state">
          <p>Enter a search term to find nodes across the work graph.</p>
        </div>
      {:else if searchState.results.length === 0}
        <div class="empty-state">
          <p>No nodes found matching "{searchState.query}".</p>
        </div>
      {:else}
        <div class="results-header">
          <span class="result-count">
            {searchState.results.length}
            {searchState.results.length === 1 ? 'result' : 'results'}
            {#if searchState.nodeTypeFilter}
              for type "{searchState.nodeTypeFilter}"
            {/if}
          </span>
        </div>

        <div class="results-list">
          {#each searchState.results as node (node.id)}
            <SearchResult {node} />
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .graph-search {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #1a1a2e;
    color: #e0e0e0;
  }

  .header {
    padding: 1rem;
    border-bottom: 1px solid #333;
  }

  .header h2 {
    margin: 0;
    font-size: 1.5rem;
    color: #e0e0e0;
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .project-selection-prompt {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #888;
  }

  .project-selection-prompt p {
    margin: 0;
    font-size: 0.875rem;
  }

  .search-bar {
    display: flex;
    gap: 0.75rem;
    padding: 1rem;
    background-color: #262641;
    border-bottom: 1px solid #333;
  }

  .search-input {
    flex: 1;
    padding: 0.625rem 1rem;
    background-color: #0f0f1e;
    color: #e0e0e0;
    border: 1px solid #333;
    border-radius: 0.375rem;
    font-size: 0.875rem;
  }

  .search-input::placeholder {
    color: #666;
  }

  .search-input:focus {
    outline: none;
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
  }

  .search-button {
    padding: 0.625rem 1.5rem;
    background-color: #3b82f6;
    color: white;
    border: none;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .search-button:hover:not(:disabled) {
    background-color: #2563eb;
  }

  .search-button:disabled {
    background-color: #666;
    cursor: not-allowed;
  }

  .filter-bar {
    display: flex;
    gap: 0.5rem;
    padding: 1rem;
    background-color: #0f0f1e;
    border-bottom: 1px solid #333;
    flex-wrap: wrap;
  }

  .filter-button {
    padding: 0.375rem 0.75rem;
    background-color: #333;
    color: #b0b0c0;
    border: 1px solid #444;
    border-radius: 9999px;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.2s;
  }

  .filter-button:hover {
    background-color: #444;
  }

  .filter-button.active {
    background-color: #3b82f6;
    color: white;
    border-color: #3b82f6;
  }

  .results-header {
    padding: 1rem;
    background-color: #0f0f1e;
    border-bottom: 1px solid #333;
  }

  .result-count {
    font-size: 0.875rem;
    color: #b0b0c0;
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1rem;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #888;
    text-align: center;
  }

  .empty-state p {
    margin: 0;
    font-size: 0.875rem;
  }
</style>
