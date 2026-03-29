<script lang="ts">
  /**
   * SearchResult component.
   * Displays a compact card showing a search result node.
   * Includes type badge, status badge, title, description, and node ID.
   */

  import { navigate } from '../router.svelte';
  import StatusBadge from './StatusBadge.svelte';
  import type { GraphNode } from '../types';

  type Props = {
    node: GraphNode;
  };

  let { node }: Props = $props();

  /**
   * Get the root goal ID from a hierarchical node ID.
   * Examples:
   *  - "ra-1234" -> "ra-1234"
   *  - "ra-1234.1" -> "ra-1234"
   *  - "ra-1234.1.2" -> "ra-1234"
   */
  function getRootGoalId(nodeId: string): string {
    const parts = nodeId.split('.');
    return parts[0];
  }

  /**
   * Navigate based on node type.
   */
  function handleClick(): void {
    const rootGoalId = getRootGoalId(node.id);

    switch (node.node_type) {
      case 'goal':
      case 'task':
        navigate(`/goals/${rootGoalId}/tasks`);
        break;
      case 'decision':
      case 'option':
        navigate(`/goals/${rootGoalId}/decisions`);
        break;
      case 'observation':
      case 'outcome':
      case 'revisit':
        navigate(`/goals/${rootGoalId}/tasks`);
        break;
      default:
        // Default to task view
        navigate(`/goals/${rootGoalId}/tasks`);
    }
  }

  /**
   * Get the color for the node type badge.
   */
  function getNodeTypeColor(type: string): string {
    switch (type) {
      case 'goal':
        return '#8b5cf6'; // purple
      case 'task':
        return '#3b82f6'; // blue
      case 'decision':
        return '#ec4899'; // pink
      case 'option':
        return '#f97316'; // orange
      case 'outcome':
        return '#10b981'; // green
      case 'observation':
        return '#06b6d4'; // cyan
      case 'revisit':
        return '#f59e0b'; // amber
      default:
        return '#6b7280'; // gray
    }
  }

  /**
   * Truncate description to 150 characters.
   */
  function truncateDescription(desc: string, maxLen: number = 150): string {
    if (desc.length > maxLen) {
      return desc.slice(0, maxLen) + '...';
    }
    return desc;
  }

  /**
   * Format node type name for display.
   */
  function formatNodeType(type: string): string {
    return type.charAt(0).toUpperCase() + type.slice(1);
  }

  const nodeTypeColor = $derived(getNodeTypeColor(node.node_type));
  const truncatedDesc = $derived(truncateDescription(node.description));
</script>

<button type="button" class="search-result" onclick={handleClick}>
  <div class="result-header">
    <div class="type-and-status">
      <span class="type-badge" style="background-color: {nodeTypeColor}">
        {formatNodeType(node.node_type)}
      </span>
      <StatusBadge status={node.status} />
    </div>
    <span class="node-id">{node.id}</span>
  </div>

  <h3 class="result-title">{node.title}</h3>

  {#if node.description}
    <p class="result-description">{truncatedDesc}</p>
  {/if}
</button>

<style>
  .search-result {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 1rem;
    background-color: #262641;
    border: 1px solid #333;
    border-radius: 0.5rem;
    cursor: pointer;
    transition: background-color 0.2s, border-color 0.2s;
    color: inherit;
    font-family: inherit;
    text-align: left;
  }

  .search-result:hover {
    background-color: #2e2e52;
    border-color: #444;
  }

  .search-result:focus {
    outline: 2px solid #3b82f6;
    outline-offset: -2px;
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }

  .type-and-status {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .type-badge {
    display: inline-block;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 500;
    color: white;
    white-space: nowrap;
  }

  .node-id {
    font-family: monospace;
    font-size: 0.75rem;
    color: #888;
  }

  .result-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: #e0e0e0;
  }

  .result-description {
    margin: 0;
    font-size: 0.875rem;
    color: #b0b0c0;
    line-height: 1.4;
  }
</style>
