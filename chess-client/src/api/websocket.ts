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
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private wasConnected = false;  // Track if we've ever been connected (to distinguish first connect vs reconnect)

  connect(token: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(WS_URL);

      this.ws.onopen = () => {
        this.send({ type: 'auth', token });
        this.reconnectAttempts = 0;
        this.startPing();

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

        if (event.code !== 1000 && this.reconnectAttempts < this.maxReconnectAttempts) {
          this.reconnectAttempts++;
          setTimeout(() => {
            const storedToken = localStorage.getItem('token');
            if (storedToken) {
              this.connect(storedToken).catch((err) => {
                console.error('WebSocket reconnection failed:', err);
                // Reconnection failed silently — the next onclose will trigger another attempt
                // if attempts remain. No user action needed.
              });
            }
          }, this.reconnectDelay * this.reconnectAttempts);
        } else if (this.reconnectAttempts >= this.maxReconnectAttempts) {
          console.error('WebSocket: max reconnection attempts reached');
        }
      };

      this.ws.onerror = () => {
        reject(new Error('WebSocket connection failed'));
      };
    });
  }

  disconnect() {
    this.stopPing();
    this.wasConnected = false;
    this.reconnectAttempts = this.maxReconnectAttempts; // Prevent auto-reconnect
    if (this.ws) {
      this.ws.close(1000);
      this.ws = null;
    }
  }

  send(message: WsClientMessage) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
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
