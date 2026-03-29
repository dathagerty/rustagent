<script lang="ts">
  /**
   * StatusBadge component.
   * Displays a colored pill badge for a node status.
   * Color mapping based on status value.
   */

  import type { NodeStatus } from '../types';

  type Props = {
    status: NodeStatus;
  };

  let { status }: Props = $props();

  /**
   * Map status to background color.
   */
  function getBackgroundColor(st: NodeStatus): string {
    switch (st) {
      case 'completed':
        return '#10b981'; // green
      case 'active':
      case 'in_progress':
        return '#3b82f6'; // blue
      case 'ready':
      case 'claimed':
        return '#14b8a6'; // teal
      case 'pending':
        return '#6b7280'; // gray
      case 'blocked':
      case 'failed':
        return '#ef4444'; // red
      case 'decided':
      case 'chosen':
        return '#a855f7'; // purple
      case 'rejected':
      case 'abandoned':
      case 'superseded':
      case 'cancelled':
        return '#f59e0b'; // orange/muted
      case 'review':
        return '#eab308'; // yellow
      default:
        return '#6b7280'; // gray as fallback
    }
  }

  /**
   * Format status for display (snake_case to Title Case).
   */
  function formatStatus(st: NodeStatus): string {
    return st
      .split('_')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  }

  const bgColor = $derived(getBackgroundColor(status));
  const displayText = $derived(formatStatus(status));
</script>

<span class="status-badge" style="background-color: {bgColor}">
  {displayText}
</span>

<style>
  .status-badge {
    display: inline-block;
    padding: 0.25rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.875rem;
    font-weight: 500;
    color: white;
    white-space: nowrap;
  }
</style>
