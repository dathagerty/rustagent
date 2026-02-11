/**
 * Reactive store for search state using Svelte 5 runes.
 * Manages search query, results, and filters.
 */

import { createApiClient } from '../api/client';
import type { GraphNode, NodeType } from '../types';

/**
 * Search state object.
 * Managed as Svelte 5 $state for reactivity.
 */
export const searchState = $state<{
  query: string;
  results: Array<GraphNode>;
  nodeTypeFilter: NodeType | null;
  loading: boolean;
  error: string | null;
}>({
  query: '',
  results: [],
  nodeTypeFilter: null,
  loading: false,
  error: null,
});

const apiClient = createApiClient();

/**
 * Execute a search for nodes.
 */
export async function executeSearch(projectId: string): Promise<void> {
  if (!searchState.query) {
    searchState.results = [];
    return;
  }

  searchState.loading = true;
  searchState.error = null;
  try {
    searchState.results = await apiClient.searchNodes(projectId, {
      query: searchState.query,
      node_type: searchState.nodeTypeFilter || undefined,
      limit: 50,
    });
  } catch (error) {
    searchState.error = error instanceof Error ? error.message : 'Unknown error';
    searchState.results = [];
  } finally {
    searchState.loading = false;
  }
}

/**
 * Update the search query.
 */
export function setQuery(q: string): void {
  searchState.query = q;
}

/**
 * Update the node type filter.
 */
export function setNodeTypeFilter(type: NodeType | null): void {
  searchState.nodeTypeFilter = type;
}

/**
 * Clear all search state.
 */
export function clearSearch(): void {
  searchState.query = '';
  searchState.results = [];
  searchState.nodeTypeFilter = null;
  searchState.error = null;
}
