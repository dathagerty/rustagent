<script lang="ts">
  /**
   * AgentStatusBadge component.
   * Displays a colored badge with agent ID (truncated) and status indicator.
   * Shows pulsing/animated indicators based on agent status.
   */

  type Props = {
    agentId: string;
    status: string;
  };

  let { agentId, status }: Props = $props();

  /**
   * Get background color based on status.
   */
  function getStatusColor(st: string): string {
    switch (st) {
      case 'spawning':
      case 'initializing':
        return '#3b82f6'; // blue
      case 'working':
        return '#10b981'; // green
      case 'reporting':
        return '#eab308'; // yellow
      case 'completed':
        return '#10b981'; // green
      case 'failed':
        return '#ef4444'; // red
      default:
        return '#6b7280'; // gray
    }
  }

  /**
   * Determine if status should have animation.
   */
  function hasAnimation(st: string): boolean {
    return st === 'spawning' || st === 'initializing' || st === 'working';
  }

  /**
   * Truncate agent ID to last 6 characters.
   */
  function truncateId(id: string): string {
    return id.length > 6 ? id.slice(-6) : id;
  }

  const bgColor = $derived(getStatusColor(status));
  const isAnimated = $derived(hasAnimation(status));
  const displayId = $derived(truncateId(agentId));
</script>

<div class="badge-wrapper">
  <span class="status-badge" class:animated={isAnimated} style="background-color: {bgColor}">
    <span class="status-dot"></span>
    <span class="agent-id">{displayId}</span>
    <span class="status-text">{status}</span>
  </span>
</div>

<style>
  .badge-wrapper {
    display: inline-flex;
    margin-right: 0.5rem;
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.375rem 0.75rem;
    border-radius: 9999px;
    font-size: 0.875rem;
    font-weight: 500;
    color: white;
    white-space: nowrap;
  }

  .status-dot {
    display: inline-block;
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background-color: currentColor;
    opacity: 0.8;
  }

  .status-badge.animated .status-dot {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .agent-id {
    font-family: monospace;
    font-size: 0.75rem;
    opacity: 0.9;
  }

  .status-text {
    font-size: 0.75rem;
    opacity: 0.85;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
</style>
