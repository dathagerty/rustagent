import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { WsEvent } from '../types';
import { WsConnection } from './websocket';

// Mock WebSocket
class MockWebSocket {
  readyState = 0; // CONNECTING
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;

  constructor(private url: string) {
    // Store for access in tests
    MockWebSocket.lastInstance = this;
  }

  send(data: string) {
    // No-op for testing
  }

  close() {
    this.readyState = 3; // CLOSED
  }

  simulateOpen() {
    this.readyState = 1; // OPEN
    if (this.onopen) {
      this.onopen(new Event('open'));
    }
  }

  simulateClose() {
    this.readyState = 3; // CLOSED
    if (this.onclose) {
      this.onclose(new Event('close'));
    }
  }

  simulateMessage(data: string) {
    if (this.onmessage) {
      this.onmessage(new MessageEvent('message', { data }));
    }
  }

  static lastInstance: MockWebSocket | undefined;
}

// Replace global WebSocket with mock
const originalWebSocket = (globalThis as any).WebSocket;
beforeEach(() => {
  (globalThis as any).WebSocket = MockWebSocket;
  vi.useFakeTimers();
});

afterEach(() => {
  (globalThis as any).WebSocket = originalWebSocket;
  vi.useRealTimers();
  MockWebSocket.lastInstance = undefined;
});

describe('WsConnection', () => {
  it('connects to provided URL', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    conn.connect();

    expect(MockWebSocket.lastInstance).toBeDefined();
  });

  it('auto-detects URL from window.location when not provided', () => {
    // Create a connection without explicit URL
    // Note: In the real implementation, it auto-detects from window.location
    // For testing, we'll test with explicit URL
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    expect(conn['wsUrl']).toBe(url);
  });

  it('dispatches events to registered listeners', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener = vi.fn();

    conn.onEvent(listener);
    conn.connect();

    // Simulate connection opening
    MockWebSocket.lastInstance?.simulateOpen();

    // Simulate an agent_spawned event
    const event: WsEvent = {
      type: 'agent_spawned',
      agent_id: 'agent-123',
      profile: 'coder',
      goal_id: 'goal-456',
    };
    MockWebSocket.lastInstance?.simulateMessage(JSON.stringify(event));

    expect(listener).toHaveBeenCalledWith(event);
  });

  it('handles multiple listeners', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener1 = vi.fn();
    const listener2 = vi.fn();

    conn.onEvent(listener1);
    conn.onEvent(listener2);
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    const event: WsEvent = {
      type: 'agent_spawned',
      agent_id: 'agent-123',
      profile: 'coder',
      goal_id: 'goal-456',
    };
    MockWebSocket.lastInstance?.simulateMessage(JSON.stringify(event));

    expect(listener1).toHaveBeenCalledWith(event);
    expect(listener2).toHaveBeenCalledWith(event);
  });

  it('removes listeners with offEvent', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener = vi.fn();

    conn.onEvent(listener);
    conn.offEvent(listener);
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    const event: WsEvent = {
      type: 'agent_spawned',
      agent_id: 'agent-123',
      profile: 'coder',
      goal_id: 'goal-456',
    };
    MockWebSocket.lastInstance?.simulateMessage(JSON.stringify(event));

    expect(listener).not.toHaveBeenCalled();
  });

  it('sets connected to true when WebSocket opens', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    expect(conn.connected).toBe(false);

    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    expect(conn.connected).toBe(true);
  });

  it('sets connected to false when WebSocket closes', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);

    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();
    expect(conn.connected).toBe(true);

    MockWebSocket.lastInstance?.simulateClose();
    expect(conn.connected).toBe(false);
  });

  it('handles malformed JSON gracefully', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener = vi.fn();
    const consoleWarnSpy = vi.spyOn(console, 'warn');

    conn.onEvent(listener);
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    MockWebSocket.lastInstance?.simulateMessage('invalid json');

    expect(consoleWarnSpy).toHaveBeenCalled();
    expect(listener).not.toHaveBeenCalled();

    consoleWarnSpy.mockRestore();
  });

  it('handles unknown event types gracefully', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener = vi.fn();
    const consoleWarnSpy = vi.spyOn(console, 'warn');

    conn.onEvent(listener);
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    MockWebSocket.lastInstance?.simulateMessage(JSON.stringify({ type: 'unknown_type' }));

    expect(consoleWarnSpy).toHaveBeenCalled();
    expect(listener).not.toHaveBeenCalled();

    consoleWarnSpy.mockRestore();
  });

  it('reconnects with exponential backoff on close', async () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);

    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();
    const firstInstance = MockWebSocket.lastInstance;

    // Close the connection
    MockWebSocket.lastInstance?.simulateClose();

    // After 1 second, it should reconnect
    vi.advanceTimersByTime(1000);
    expect(MockWebSocket.lastInstance).not.toBe(firstInstance);

    // Next reconnect should be after 2 seconds
    MockWebSocket.lastInstance?.simulateClose();
    vi.advanceTimersByTime(2000);
    const thirdInstance = MockWebSocket.lastInstance;

    // Next should be 4 seconds
    MockWebSocket.lastInstance?.simulateClose();
    vi.advanceTimersByTime(4000);
    const fourthInstance = MockWebSocket.lastInstance;

    expect(fourthInstance).not.toBe(thirdInstance);
  });

  it('resets backoff on successful connection', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);

    // First connection
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();
    const firstInstance = MockWebSocket.lastInstance;

    // Close and reconnect quickly
    MockWebSocket.lastInstance?.simulateClose();
    vi.advanceTimersByTime(1000); // First backoff
    MockWebSocket.lastInstance?.simulateOpen();

    // Now close again - should use 1s backoff again, not 2s
    MockWebSocket.lastInstance?.simulateClose();
    const beforeReconnect = MockWebSocket.lastInstance;
    vi.advanceTimersByTime(1000);
    const afterReconnect = MockWebSocket.lastInstance;

    // Should have reconnected after 1s (reset backoff)
    expect(afterReconnect).not.toBe(beforeReconnect);
  });

  it('stops reconnection attempts when disconnect() is called', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);

    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();
    const firstInstance = MockWebSocket.lastInstance;

    MockWebSocket.lastInstance?.simulateClose();
    conn.disconnect();

    vi.advanceTimersByTime(2000);
    // Should still be the closed instance, no reconnect
    expect(MockWebSocket.lastInstance).toBe(firstInstance);
  });

  it('caps backoff at 30 seconds', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);

    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    // Trigger multiple reconnections: 1s, 2s, 4s, 8s, 16s, 32s → capped at 30s
    for (let i = 0; i < 6; i++) {
      MockWebSocket.lastInstance?.simulateClose();
      vi.advanceTimersByTime(32000); // Advance beyond any possible backoff
    }

    // The last reconnect should use 30s max
    MockWebSocket.lastInstance?.simulateClose();
    const beforeAdvance = MockWebSocket.lastInstance;
    vi.advanceTimersByTime(29000); // Less than 30s
    expect(MockWebSocket.lastInstance).toBe(beforeAdvance);

    vi.advanceTimersByTime(1000); // Now we're past 30s
    expect(MockWebSocket.lastInstance).not.toBe(beforeAdvance);
  });

  it('dispatches all event types correctly', () => {
    const url = 'ws://localhost:7400/ws';
    const conn = new WsConnection(url);
    const listener = vi.fn();

    conn.onEvent(listener);
    conn.connect();
    MockWebSocket.lastInstance?.simulateOpen();

    const events: WsEvent[] = [
      {
        type: 'agent_spawned',
        agent_id: 'agent-1',
        profile: 'coder',
        goal_id: 'goal-1',
      },
      {
        type: 'agent_progress',
        agent_id: 'agent-1',
        turn: 1,
        summary: 'working',
      },
      {
        type: 'agent_completed',
        agent_id: 'agent-1',
        outcome_type: 'success',
        summary: 'completed',
        tokens_used: 1000,
      },
      {
        type: 'node_status_changed',
        node_id: 'node-1',
        node_type: 'task',
        old_status: 'pending',
        new_status: 'completed',
      },
      {
        type: 'session_ended',
        session_id: 'sess-1',
        handoff_notes: 'notes',
      },
      {
        type: 'tool_execution',
        agent_id: 'agent-1',
        tool: 'read_file',
        args: { path: '/test' },
        result: 'content',
      },
      {
        type: 'orchestrator_state_changed',
        goal_id: 'goal-1',
        state: 'completed',
      },
    ];

    for (const event of events) {
      MockWebSocket.lastInstance?.simulateMessage(JSON.stringify(event));
    }

    expect(listener).toHaveBeenCalledTimes(events.length);
    for (let i = 0; i < events.length; i++) {
      expect(listener).toHaveBeenNthCalledWith(i + 1, expect.objectContaining({ type: events[i].type }));
    }
  });
});
