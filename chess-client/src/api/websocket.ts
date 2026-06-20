import type { WsClientMessage, WsServerMessage } from '../types';

type MessageHandler = (message: WsServerMessage) => void;
type ConnectionHandler = () => void;

const WS_URL = (() => {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = window.location.host;
  return `${protocol}//${host}/ws`;
})();

class WebSocketService {
  private ws: WebSocket | null = null;
  private messageHandlers: Set<MessageHandler> = new Set();
  private connectHandlers: Set<ConnectionHandler> = new Set();
  private disconnectHandlers: Set<ConnectionHandler> = new Set();
  private reconnectHandlers: Set<ConnectionHandler> = new Set();
  private reconnectAttempts = 0;
  private reconnectDelay = 1000;  // Base delay in ms (used for exponential backoff)
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private wasConnected = false;
  // Message queue for offline sends — messages are replayed on reconnect
  private pendingMessages: WsClientMessage[] = [];

  /** Check if WebSocket is currently connected and ready to send messages. */
  get isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  connect(token: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(WS_URL);

      this.ws.onopen = () => {
        this.send({ type: 'auth', token });
        this.reconnectAttempts = 0;
        this.startPing();

        // Replay any queued messages that were sent while offline
        for (const msg of this.pendingMessages) {
          this.ws.send(JSON.stringify(msg));
        }
        this.pendingMessages = [];

        if (this.wasConnected) {
          // This is a reconnection, not the first connection
          this.reconnectHandlers.forEach(h => h());
        } else {
          this.wasConnected = true;
          this.connectHandlers.forEach(h => h());
        }
        resolve();
      };

      this.ws.onmessage = (event) => {
        try {
          const message: WsServerMessage = JSON.parse(event.data);
          this.messageHandlers.forEach(h => h(message));
        } catch {
          // ignore parse errors
        }
      };

      this.ws.onclose = (event) => {
        this.stopPing();
        this.disconnectHandlers.forEach(h => h());

        // Reconnect on any abnormal close (not intentional disconnect via code 1000)
        if (event.code !== 1000) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = () => {
        reject(new Error('WebSocket connection failed'));
      };
    });
  }

  /// Schedule a reconnection attempt with exponential backoff + jitter.
  /// The delay doubles each attempt: 1s, 2s, 4s, 8s, 16s, 32s, ...
  /// capped at 30 seconds. A random jitter of 0–500ms is added to
  /// prevent thundering herd on server restart.
  /// Reconnection continues indefinitely — never gives up.
  private scheduleReconnect() {
    if (this.reconnectTimer) return;  // Already scheduled

    // Exponential backoff: 2^attempts * baseDelay, capped at 30s
    const backoff = Math.min(
      Math.pow(2, this.reconnectAttempts) * this.reconnectDelay,
      30000
    );
    // Random jitter: 0 to 500ms to avoid thundering herd
    const jitter = Math.random() * 500;

    this.reconnectAttempts++;
    const delay = backoff + jitter;

    console.log(`WebSocket: reconnecting in ${Math.round(delay)}ms (attempt ${this.reconnectAttempts})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      const storedToken = localStorage.getItem('token');
      if (storedToken) {
        this.connect(storedToken).catch((err) => {
          console.error('WebSocket reconnection failed:', err);
          // Schedule another attempt — never give up
          this.scheduleReconnect();
        });
      }
    }, delay);
  }

  disconnect() {
    // Cancel any pending reconnect
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.stopPing();
    this.wasConnected = false;
    this.reconnectAttempts = 0;
    this.pendingMessages = [];
    if (this.ws) {
      this.ws.close(1000);
      this.ws = null;
    }
  }

  send(message: WsClientMessage) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      // Queue the message for later — it will be replayed on reconnect
      this.pendingMessages.push(message);
    }
  }

  joinGame(gameId: string) {
    this.send({ type: 'join_game', game_id: gameId });
  }

  leaveGame(gameId: string) {
    this.send({ type: 'leave_game', game_id: gameId });
  }

  makeMove(gameId: string, from: string, to: string) {
    this.send({ type: 'make_move', game_id: gameId, from, to });
  }

  resign(gameId: string) {
    this.send({ type: 'resign', game_id: gameId });
  }

  offerDraw(gameId: string) {
    this.send({ type: 'offer_draw', game_id: gameId });
  }

  respondDraw(gameId: string, accept: boolean) {
    this.send({ type: 'respond_draw', game_id: gameId, accept });
  }

  onMessage(handler: MessageHandler): () => void {
    this.messageHandlers.add(handler);
    return () => this.messageHandlers.delete(handler);
  }

  onConnect(handler: ConnectionHandler): () => void {
    this.connectHandlers.add(handler);
    return () => this.connectHandlers.delete(handler);
  }

  onDisconnect(handler: ConnectionHandler): () => void {
    this.disconnectHandlers.add(handler);
    return () => this.disconnectHandlers.delete(handler);
  }

  onReconnect(handler: ConnectionHandler): () => void {
    this.reconnectHandlers.add(handler);
    return () => this.reconnectHandlers.delete(handler);
  }

  private startPing() {
    this.pingInterval = setInterval(() => {
      this.send({ type: 'ping' });
    }, 30000);
  }

  private stopPing() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }
}

export const wsService = new WebSocketService();
