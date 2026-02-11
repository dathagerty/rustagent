<script lang="ts">
  /**
   * PriorityBadge component.
   * Displays a colored indicator for a priority level.
   * Renders nothing if priority is null.
   */

  import type { Priority } from '../types';

  type Props = {
    priority: Priority | null;
  };

  let { priority }: Props = $props();

  /**
   * Map priority to background color.
   */
  function getBackgroundColor(p: Priority): string {
    switch (p) {
      case 'critical':
        return '#ef4444'; // red
      case 'high':
        return '#f97316'; // orange
      case 'medium':
        return '#eab308'; // yellow
      case 'low':
        return '#6b7280'; // gray
      default:
        return '#6b7280'; // gray as fallback
    }
  }

  /**
   * Format priority for display (lowercase to Title Case).
   */
  function formatPriority(p: Priority): string {
    return p.charAt(0).toUpperCase() + p.slice(1);
  }

  const bgColor = $derived(priority ? getBackgroundColor(priority) : '');
  const displayText = $derived(priority ? formatPriority(priority) : '');
</script>

{#if priority}
  <span class="priority-badge" style="background-color: {bgColor}">
    {displayText}
  </span>
{/if}

<style>
  .priority-badge {
    display: inline-block;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.875rem;
    font-weight: 500;
    color: white;
    white-space: nowrap;
  }
</style>
