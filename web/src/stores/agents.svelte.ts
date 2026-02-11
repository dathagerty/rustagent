/**
 * Reactive store for agents and event feed using Svelte 5 runes.
 * Manages active agents and WebSocket event stream.
 */

import { createApiClient } from '../api/client';
import type { ActiveAgent, WsEvent } from '../types';

/**
 * Agents state object.
 * Managed as Svelte 5 $state for reactivity.
 * Event feed is a ring buffer (max 200 items, newest at index 0).
 */
export const agentsState = $state<{
  activeAgents: Array<ActiveAgent>;
  eventFeed: Array<WsEvent>;
  loading: boolean;
}>({
  activeAgents: [],
  eventFeed: [],
  loading: false,
});

const apiClient = createApiClient();
const MAX_FEED_SIZE = 200;

/**
 * Load active agents for a goal.
 */
export async function loadAgents(goalId: string): Promise<void> {
  agentsState.loading = true;
  try {
    agentsState.activeAgents = await apiClient.listAgents(goalId);
  } finally {
    agentsState.loading = false;
  }
}

/**
 * Add an event to the feed.
 * Maintains a ring buffer by trimming to MAX_FEED_SIZE.
 * Newest events appear at index 0.
 */
export function addEvent(event: WsEvent): void {
  agentsState.eventFeed.unshift(event);
  if (agentsState.eventFeed.length > MAX_FEED_SIZE) {
    agentsState.eventFeed.pop();
  }
}

/**
 * Clear the event feed.
 */
export function clearFeed(): void {
  agentsState.eventFeed = [];
}

/**
 * Handle WebSocket events.
 * - agent_spawned: add placeholder to activeAgents
 * - agent_progress: add to feed
 * - agent_completed: remove from activeAgents and add to feed
 * - tool_execution: add to feed
 * Agent-related events are added to the event feed.
 */
export function handleWsEvent(event: WsEvent): void {
  if (event.type === 'agent_spawned') {
    // Add placeholder to activeAgents
    const placeholder: ActiveAgent = {
      agent_id: event.agent_id,
      task_id: '', // Will be updated when agent takes a task
      task_title: 'Initializing...',
      task_status: 'pending',
    };
    agentsState.activeAgents.push(placeholder);
    addEvent(event);
  } else if (event.type === 'agent_progress') {
    addEvent(event);
  } else if (event.type === 'agent_completed') {
    // Remove from activeAgents
    agentsState.activeAgents = agentsState.activeAgents.filter(
      (a) => a.agent_id !== event.agent_id
    );
    addEvent(event);
  } else if (event.type === 'tool_execution') {
    addEvent(event);
  }
}
