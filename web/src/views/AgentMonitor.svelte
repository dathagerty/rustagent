<script lang="ts">
  /**
   * AgentMonitor view component.
   * Shows real-time feed of agent activity for a goal via WebSocket events.
   * Displays active agents and event feed with auto-scroll.
   */

  import { getCurrentRoute } from '../router.svelte';
  import { agentsState, loadAgents, clearFeed } from '../stores/agents.svelte';
  import { formatRelativeTime } from '../lib/date-formatting';
  import AgentStatusBadge from '../components/AgentStatusBadge.svelte';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import type { WsEvent } from '../types';

  let currentRoute = $derived(getCurrentRoute());
  let goalId = $derived(currentRoute.params.goalId);

  let feedContainer = $state<HTMLDivElement | null>(null);
  let autoScrollEnabled = $state(true);
  let relativeTimeCounter = $state(0);

  /**
   * Load active agents on mount or when goalId changes.
   */
  $effect(() => {
    const id = goalId;
    if (id) {
      loadAgents(id);
    }
  });

  /**
   * Auto-scroll to bottom when feed updates.
   */
  $effect(() => {
    // Read reactive dependencies
    const feedLength = agentsState.eventFeed.length;
    const shouldAutoScroll = autoScrollEnabled;

    if (shouldAutoScroll && feedContainer) {
      // Use setTimeout to ensure DOM is updated
      setTimeout(() => {
        feedContainer.scrollTop = feedContainer.scrollHeight;
      }, 0);
    }
  });

  /**
   * Update relative times every 10 seconds.
   */
  $effect(() => {
    const interval = setInterval(() => {
      relativeTimeCounter += 1;
    }, 10000);

    return () => {
      clearInterval(interval);
    };
  });

  /**
   * Format event for display based on type.
   */
  function formatEventMessage(event: WsEvent): string {
    switch (event.type) {
      case 'agent_spawned':
        return `Agent ${event.agent_id} spawned with profile ${event.profile} for goal ${event.goal_id}`;
      case 'agent_progress':
        return `Agent ${event.agent_id} (turn ${event.turn}): ${event.summary}`;
      case 'agent_completed':
        return `Agent ${event.agent_id} completed: ${event.summary}${event.tokens_used ? ` (${event.tokens_used} tokens)` : ''}`;
      case 'tool_execution':
        return `Agent ${event.agent_id} → ${event.tool}(${JSON.stringify(event.args).slice(0, 100)}) = ${String(event.result).slice(0, 100)}`;
      default:
        return `[${event.type}] ${JSON.stringify(event)}`;
    }
  }

  /**
   * Get event background color based on type.
   */
  function getEventBgColor(event: WsEvent): string {
    switch (event.type) {
      case 'agent_spawned':
        return 'rgba(59, 130, 246, 0.1)'; // blue
      case 'agent_completed':
        return event.outcome_type === 'failure' ? 'rgba(239, 68, 68, 0.1)' : 'rgba(16, 185, 129, 0.1)';
      case 'tool_execution':
        return 'rgba(55, 65, 81, 0.2)'; // gray
      default:
        return 'rgba(107, 114, 128, 0.1)'; // gray
    }
  }

  /**
   * Get event text color based on type.
   */
  function getEventTextColor(event: WsEvent): string {
    switch (event.type) {
      case 'agent_spawned':
        return '#93c5fd'; // light blue
      case 'agent_completed':
        return event.outcome_type === 'failure' ? '#fca5a5' : '#86efac'; // light red or green
      case 'tool_execution':
        return '#d1d5db'; // light gray
      default:
        return '#9ca3af'; // muted gray
    }
  }

  /**
   * Get event timestamp with relative time update trigger.
   */
  function getEventTime(event: WsEvent): string {
    const timestamp = 'created_at' in event ? event.created_at : new Date().toISOString();
    return formatRelativeTime(timestamp);
  }
</script>

<div class="agent-monitor">
  <div class="header">
    <h2>Agent Monitor</h2>
  </div>

  <div class="content">
    {#if agentsState.loading}
      <LoadingSpinner />
    {:else if goalId}
      <div class="active-agents-bar">
        <div class="agents-count">
          <strong>{agentsState.activeAgents.length}</strong>
          {agentsState.activeAgents.length === 1 ? 'agent' : 'agents'} running
        </div>
        <div class="agents-list">
          {#each agentsState.activeAgents as agent (agent.agent_id)}
            <AgentStatusBadge agentId={agent.agent_id} status="working" />
          {/each}
        </div>
      </div>

      <div class="feed-controls">
        <button
          class="btn-pause"
          onclick={() => {
            autoScrollEnabled = !autoScrollEnabled;
          }}
        >
          {autoScrollEnabled ? 'Pause' : 'Resume'} auto-scroll
        </button>
        <button class="btn-clear" onclick={() => clearFeed()}>
          Clear feed
        </button>
      </div>

      <div class="feed-container" bind:this={feedContainer}>
        {#if agentsState.eventFeed.length === 0}
          <div class="empty-state">
            <p>No agent activity yet. Events will appear here as agents run.</p>
          </div>
        {/if}
        {#each agentsState.eventFeed as event (event.type + Math.random())}
          <div
            class="event-item"
            style="background-color: {getEventBgColor(event)}; color: {getEventTextColor(event)}"
          >
            <span class="event-time">{getEventTime(event)}</span>
            <span class="event-message">{formatEventMessage(event)}</span>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-state">
        <p>No goal ID provided. Navigate to a goal first.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .agent-monitor {
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

  .active-agents-bar {
    padding: 1rem;
    background-color: #262641;
    border-bottom: 1px solid #333;
  }

  .agents-count {
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
    color: #b0b0c0;
  }

  .agents-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .feed-controls {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background-color: #0f0f1e;
    border-bottom: 1px solid #333;
  }

  .btn-pause,
  .btn-clear {
    padding: 0.375rem 0.75rem;
    background-color: #333;
    color: #e0e0e0;
    border: 1px solid #444;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .btn-pause:hover,
  .btn-clear:hover {
    background-color: #444;
  }

  .feed-container {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }

  .event-item {
    display: flex;
    gap: 1rem;
    padding: 0.75rem;
    margin-bottom: 0.5rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    border-left: 3px solid currentColor;
  }

  .event-time {
    flex-shrink: 0;
    min-width: 60px;
    color: #888;
    font-size: 0.75rem;
    text-align: right;
  }

  .event-message {
    flex: 1;
    word-break: break-word;
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
