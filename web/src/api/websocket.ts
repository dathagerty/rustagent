/**
 * WebSocket connection handler for Rustagent V2 daemon.
 * Manages persistent WebSocket connection to /ws with automatic reconnection
 * and event dispatching via callbacks.
 */

import type { WsEvent } from '../types';

/**
 * Manages a WebSocket connection to the daemon.
 * Handles automatic reconnection with exponential backoff.
 */
export class WsConnection {
  private ws: WebSocket | null = null;
  private listeners: Set<(event: WsEvent) => void> = new Set();
  private wsUrl: string;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private backoffMs: number = 1000; // Start at 1 second
  private shouldReconnect: boolean = false;

  /**
   * Constructor.
   * @param wsUrl URL for the WebSocket. If not provided, auto-detects from window.location.
   *              Use 'ws://' or 'wss://' based on page protocol, pointing to '/ws'.
   */
  constructor(wsUrl?: string) {
    if (wsUrl) {
      this.wsUrl = wsUrl;
    } else {
      // Auto-detect from window.location (browser environment)
      if (typeof window !== 'undefined' && window.location) {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const host = window.location.host;
        this.wsUrl = `${protocol}//${host}/ws`;
      } else {
        // Fallback for non-browser environments
        this.wsUrl = 'ws://localhost/ws';
      }
    }
  }

  /**
   * Current connection state.
   */
  get connected(): boolean {
    return this.ws !== null && this.ws.readyState === 1; // WebSocket.OPEN = 1
  }

  /**
   * Register a callback for all WebSocket events.
   */
  onEvent(callback: (event: WsEvent) => void): void {
    this.listeners.add(callback);
  }

  /**
   * Unregister a callback.
   */
  offEvent(callback: (event: WsEvent) => void): void {
    this.listeners.delete(callback);
  }

  /**
   * Open the WebSocket connection.
   * Automatically handles reconnection on close/error.
   */
  connect(): void {
    if (this.ws) {
      return; // Already connected or connecting
    }

    this.shouldReconnect = true;
    this.ws = new WebSocket(this.wsUrl);

    this.ws.onopen = () => {
      // Reset backoff on successful connection
      this.backoffMs = 1000;
    };

    this.ws.onmessage = (event: MessageEvent) => {
      this.handleMessage(event.data);
    };

    this.ws.onerror = () => {
      // Error will trigger onclose, so we don't need to handle reconnection here
    };

    this.ws.onclose = () => {
      this.ws = null;
      if (this.shouldReconnect) {
        this.scheduleReconnect();
      }
    };
  }

  /**
   * Close the WebSocket connection and stop reconnection attempts.
   */
  disconnect(): void {
    this.shouldReconnect = false;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  /**
   * Schedule a reconnection attempt with exponential backoff.
   * Backoff: 1s, 2s, 4s, 8s, 16s, 30s max.
   */
  private scheduleReconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
    }

    // Cap backoff at 30 seconds
    const delayMs = Math.min(this.backoffMs, 30000);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.shouldReconnect) {
        // Double backoff for next attempt (up to cap)
        if (this.backoffMs < 30000) {
          this.backoffMs *= 2;
        }
        this.connect();
      }
    }, delayMs);
  }

  /**
   * Handle incoming message from WebSocket.
   * Parses JSON, validates event shape, and dispatches to listeners.
   */
  private handleMessage(data: string): void {
    try {
      const parsed = JSON.parse(data);

      // Validate that it has a type field
      if (!parsed.type || typeof parsed.type !== 'string') {
        console.warn('WebSocket message missing or invalid type field:', parsed);
        return;
      }

      // Type-narrow to WsEvent based on type field
      // List of known event types for forward compatibility check
      const knownTypes = [
        'agent_spawned',
        'agent_progress',
        'agent_completed',
        'node_created',
        'node_status_changed',
        'edge_created',
        'session_ended',
        'tool_execution',
        'orchestrator_state_changed',
      ];

      if (!knownTypes.includes(parsed.type)) {
        console.warn('WebSocket message with unknown event type:', parsed.type);
        return;
      }

      // Dispatch to all listeners
      const event = parsed as WsEvent;
      for (const listener of this.listeners) {
        listener(event);
      }
    } catch (error) {
      console.warn('Failed to parse WebSocket message:', data, error);
    }
  }
}

/**
 * Singleton-style factory for creating a WebSocket connection.
 */
export function createWsConnection(wsUrl?: string): WsConnection {
  return new WsConnection(wsUrl);
}
