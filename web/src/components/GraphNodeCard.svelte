<script lang="ts">
  /**
   * GraphNodeCard component.
   * Displays a graph node's full information in a card layout.
   * Shows title, status, priority, agent assignment, blocked reason,
   * description, metadata (acceptance criteria), and timestamps.
   */

  import type { GraphNode } from '../types';
  import StatusBadge from './StatusBadge.svelte';
  import PriorityBadge from './PriorityBadge.svelte';
  import { formatDate } from '../lib/date-formatting';

  type Props = {
    node: GraphNode;
    onclick?: () => void;
  };

  let { node, onclick }: Props = $props();

  let showFullDescription = $state(false);

  /**
   * Format node type for display (lowercase to Title Case).
   */
  function formatNodeType(nodeType: string): string {
    return nodeType.charAt(0).toUpperCase() + nodeType.slice(1);
  }

  /**
   * Truncate description to 2-3 lines (approximately 150 characters).
   */
  function truncateDescription(text: string): string {
    const maxLength = 150;
    if (text.length > maxLength) {
      return text.substring(0, maxLength).trim() + '...';
    }
    return text;
  }

  /**
   * Format a relative timestamp (e.g., "2 hours ago" or "Feb 11, 2026").
   */
  function formatTimestamp(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      const diffHours = Math.floor(diffMs / 3600000);
      const diffDays = Math.floor(diffMs / 86400000);

      if (diffMins < 1) return 'just now';
      if (diffMins < 60) return `${diffMins} minute${diffMins !== 1 ? 's' : ''} ago`;
      if (diffHours < 24) return `${diffHours} hour${diffHours !== 1 ? 's' : ''} ago`;
      if (diffDays < 7) return `${diffDays} day${diffDays !== 1 ? 's' : ''} ago`;
      return formatDate(dateStr);
    } catch {
      return formatDate(dateStr);
    }
  }

  const nodeTypeFormatted = $derived(formatNodeType(node.node_type));
  const descriptionDisplay = $derived(
    showFullDescription ? node.description : truncateDescription(node.description)
  );
  const shouldShowMoreToggle = $derived(node.description.length > 150);
  const createdDisplay = $derived(formatTimestamp(node.created_at));
</script>

<div class="card" {onclick}>
  <div class="card-header">
    <h3 class="title">{node.title}</h3>
    <div class="badges">
      <StatusBadge status={node.status} />
      <PriorityBadge priority={node.priority} />
    </div>
  </div>

  <div class="meta-row">
    <span class="node-type">{nodeTypeFormatted}</span>
    {#if node.assigned_to}
      <span class="agent-chip">{node.assigned_to}</span>
    {/if}
  </div>

  {#if node.blocked_reason}
    <div class="blocked-reason">
      <strong>Blocked:</strong> {node.blocked_reason}
    </div>
  {/if}

  {#if node.description}
    <div class="description">
      <p>{descriptionDisplay}</p>
      {#if shouldShowMoreToggle}
        <button
          class="show-more-btn"
          onclick={() => {
            showFullDescription = !showFullDescription;
          }}
        >
          {showFullDescription ? 'Show Less' : 'Show More'}
        </button>
      {/if}
    </div>
  {/if}

  {#if node.metadata && Object.keys(node.metadata).length > 0}
    <div class="metadata-section">
      {#if node.metadata.acceptance_criteria}
        <div class="acceptance-criteria">
          <h4>Acceptance Criteria</h4>
          <ul>
            {#each node.metadata.acceptance_criteria.split('\n').filter((line) => line.trim()) as criterion}
              <li>
                <input type="checkbox" disabled />
                {criterion.trim().replace(/^[-*]\s*/, '')}
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if Object.keys(node.metadata).some((k) => k !== 'acceptance_criteria')}
        <div class="other-metadata">
          {#each Object.entries(node.metadata) as [key, value]}
            {#if key !== 'acceptance_criteria'}
              <div class="metadata-pair">
                <strong>{key}:</strong>
                <span>{value}</span>
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <div class="timestamps">
    <div class="timestamp">
      <span class="label">Created:</span>
      <span class="value">{createdDisplay}</span>
    </div>
    {#if node.started_at}
      <div class="timestamp">
        <span class="label">Started:</span>
        <span class="value">{formatTimestamp(node.started_at)}</span>
      </div>
    {/if}
    {#if node.completed_at}
      <div class="timestamp">
        <span class="label">Completed:</span>
        <span class="value">{formatTimestamp(node.completed_at)}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .card {
    background: white;
    border: 1px solid #e5e7eb;
    border-radius: 0.5rem;
    padding: 1.5rem;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .card:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
    border-color: #d1d5db;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
    gap: 1rem;
  }

  .title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    color: #111827;
    flex: 1;
  }

  .badges {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .meta-row {
    display: flex;
    gap: 1rem;
    align-items: center;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }

  .node-type {
    color: #6b7280;
    font-size: 0.8125rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .agent-chip {
    background-color: #f0f4f8;
    border: 1px solid #cbd5e1;
    padding: 0.25rem 0.75rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    font-weight: 500;
    color: #334155;
  }

  .blocked-reason {
    background-color: #fee2e2;
    border-left: 4px solid #ef4444;
    padding: 0.75rem;
    margin-bottom: 1rem;
    border-radius: 0.375rem;
    color: #7f1d1d;
    font-size: 0.875rem;
  }

  .description {
    margin-bottom: 1rem;
  }

  .description p {
    margin: 0 0 0.5rem 0;
    color: #374151;
    line-height: 1.5;
    font-size: 0.875rem;
  }

  .show-more-btn {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.875rem;
    font-weight: 500;
    padding: 0;
    text-decoration: underline;
  }

  .show-more-btn:hover {
    color: #2563eb;
  }

  .metadata-section {
    margin-bottom: 1rem;
    border-top: 1px solid #e5e7eb;
    padding-top: 1rem;
  }

  .acceptance-criteria h4 {
    margin: 0 0 0.5rem 0;
    font-size: 0.875rem;
    font-weight: 600;
    color: #374151;
  }

  .acceptance-criteria ul {
    margin: 0;
    padding-left: 1.5rem;
    list-style: none;
  }

  .acceptance-criteria li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
    color: #374151;
  }

  .acceptance-criteria input {
    cursor: default;
    width: 1rem;
    height: 1rem;
  }

  .other-metadata {
    margin-top: 1rem;
  }

  .metadata-pair {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
    color: #374151;
  }

  .metadata-pair strong {
    min-width: 8rem;
    color: #6b7280;
  }

  .timestamps {
    border-top: 1px solid #e5e7eb;
    padding-top: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.8125rem;
    color: #6b7280;
  }

  .timestamp {
    display: flex;
    gap: 0.75rem;
  }

  .timestamp .label {
    font-weight: 500;
    min-width: 6rem;
  }

  .timestamp .value {
    color: #374151;
  }
</style>
