<script lang="ts">
  /**
   * SessionHistory view component.
   * Shows past sessions for a goal with handoff notes.
   * Displays sessions with start/end times, participating agents, and handoff notes.
   */

  import { getCurrentRoute } from '../router.svelte';
  import { apiClient } from '../api';
  import { formatDate } from '../lib/date-formatting';
  import LoadingSpinner from '../components/LoadingSpinner.svelte';
  import ErrorMessage from '../components/ErrorMessage.svelte';
  import type { Session } from '../types';

  let currentRoute = $derived(getCurrentRoute());
  let goalId = $derived(currentRoute.params.goalId);

  let sessions: Array<Session> = $state([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let expandedSessionId = $state<string | null>(null);

  /**
   * Load sessions on mount or when goalId changes.
   */
  $effect(() => {
    const id = goalId;
    if (id) {
      loadSessions(id);
    }
  });

  /**
   * Async function to load sessions.
   */
  async function loadSessions(id: string): Promise<void> {
    try {
      loading = true;
      error = null;
      const fetchedSessions = await apiClient.listSessions(id);
      // Sort by started_at descending (newest first)
      sessions = fetchedSessions.sort((a, b) => {
        return new Date(b.started_at).getTime() - new Date(a.started_at).getTime();
      });
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to load sessions';
    } finally {
      loading = false;
    }
  }

  /**
   * Toggle session expansion.
   */
  function toggleSession(id: string): void {
    expandedSessionId = expandedSessionId === id ? null : id;
  }

  /**
   * Calculate duration between start and end times.
   */
  function calculateDuration(started: string, ended: string | null): string {
    if (!ended) {
      return 'In progress';
    }
    const startTime = new Date(started).getTime();
    const endTime = new Date(ended).getTime();
    const seconds = Math.floor((endTime - startTime) / 1000);

    if (seconds < 60) {
      return `${seconds}s`;
    }
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
      return `${minutes}m`;
    }
    const hours = Math.floor(minutes / 60);
    return `${hours}h ${minutes % 60}m`;
  }

  /**
   * Truncate session ID for display.
   */
  function truncateId(id: string): string {
    return id.length > 8 ? id.slice(0, 8) + '...' : id;
  }

  /**
   * Format handoff notes for display.
   */
  function formatHandoffNotes(notes: string | null): string {
    return notes || 'No handoff notes for this session.';
  }
</script>

<div class="session-history">
  <div class="header">
    <h2>Session History</h2>
  </div>

  <div class="content">
    {#if loading}
      <LoadingSpinner />
    {:else if error}
      <ErrorMessage message={error} />
    {:else if sessions.length === 0}
      <div class="empty-state">
        <p>No sessions found for this goal.</p>
      </div>
    {:else}
      <div class="sessions-list">
        {#each sessions as session (session.id)}
          <div class="session-card">
            <button
              type="button"
              class="session-header"
              onclick={() => toggleSession(session.id)}
              aria-expanded={expandedSessionId === session.id}
            >
              <div class="session-id-info">
                <h3>{truncateId(session.id)}</h3>
                <span class="session-time">
                  {formatDate(session.started_at)}
                  {#if session.ended_at}
                    → {formatDate(session.ended_at)}
                  {/if}
                </span>
              </div>
              <div class="session-meta">
                <span class="duration">{calculateDuration(session.started_at, session.ended_at)}</span>
                <span class="agents-count">{session.agent_ids.length} agent{session.agent_ids.length === 1 ? '' : 's'}</span>
                <span class="expand-icon" class:expanded={expandedSessionId === session.id}>▼</span>
              </div>
            </button>

            {#if session.summary}
              <div class="session-summary">
                {session.summary}
              </div>
            {/if}

            {#if expandedSessionId === session.id}
              <div class="session-details">
                <div class="agents-section">
                  <strong>Participating Agents:</strong>
                  <div class="agents-chips">
                    {#each session.agent_ids as agentId (agentId)}
                      <span class="agent-chip">{agentId}</span>
                    {/each}
                  </div>
                </div>

                <div class="handoff-notes-section">
                  <strong>Handoff Notes:</strong>
                  <pre class="handoff-notes">{formatHandoffNotes(session.handoff_notes)}</pre>
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .session-history {
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
    overflow-y: auto;
    padding: 1rem;
  }

  .sessions-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .session-card {
    background-color: #262641;
    border: 1px solid #333;
    border-radius: 0.5rem;
    overflow: hidden;
  }

  .session-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background-color: #0f0f1e;
    cursor: pointer;
    border: none;
    color: inherit;
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    transition: background-color 0.2s;
    width: 100%;
  }

  .session-header:hover {
    background-color: #1a1a2e;
  }

  .session-header:focus {
    outline: 2px solid #3b82f6;
    outline-offset: -2px;
  }

  .session-id-info h3 {
    margin: 0 0 0.25rem 0;
    font-size: 1rem;
    font-family: monospace;
    color: #e0e0e0;
  }

  .session-time {
    font-size: 0.875rem;
    color: #888;
  }

  .session-meta {
    display: flex;
    align-items: center;
    gap: 1rem;
    text-align: right;
  }

  .duration {
    font-size: 0.875rem;
    color: #b0b0c0;
    min-width: 50px;
  }

  .agents-count {
    font-size: 0.875rem;
    color: #b0b0c0;
    min-width: 60px;
  }

  .expand-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    color: #888;
    transition: transform 0.2s;
  }

  .expand-icon.expanded {
    transform: rotate(180deg);
  }

  .session-summary {
    padding: 0 1rem 0.75rem 1rem;
    font-size: 0.875rem;
    color: #b0b0c0;
    border-top: 1px solid #333;
  }

  .session-details {
    padding: 1rem;
    background-color: #262641;
    border-top: 1px solid #333;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .agents-section,
  .handoff-notes-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .agents-section > strong,
  .handoff-notes-section > strong {
    font-size: 0.875rem;
    color: #e0e0e0;
  }

  .agents-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .agent-chip {
    display: inline-block;
    padding: 0.25rem 0.75rem;
    background-color: #3b82f6;
    color: white;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-family: monospace;
  }

  .handoff-notes {
    margin: 0;
    padding: 0.75rem;
    background-color: #0f0f1e;
    border: 1px solid #333;
    border-radius: 0.375rem;
    color: #d1d5db;
    font-size: 0.8125rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-x: auto;
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
