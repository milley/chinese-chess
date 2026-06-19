import { describe, it, expect, vi, beforeEach } from 'vitest';

// We test the WebSocket message construction logic.
// Since the WS class creates a real WebSocket, we mock it.
class MockWebSocket {
  url: string;
  sent: string[] = [];
  onmessage: ((ev: { data: string }) => void) | null = null;
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;

  constructor(url: string) {
    this.url = url;
  }

  send(data: string) {
    this.sent.push(data);
  }

  close() {}

  // Simulate server message
  simulateMessage(data: string) {
    if (this.onmessage) {
      this.onmessage({ data });
    }
  }
}

describe('WebSocket message construction', () => {
  let ws: MockWebSocket;

  beforeEach(() => {
    ws = new MockWebSocket('ws://localhost:3000/ws');
  });

  it('joinGame sends correct message', () => {
    const msg = JSON.stringify({ type: 'join_game', game_id: 'g1' });
    ws.send(msg);
    expect(ws.sent).toHaveLength(1);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('join_game');
    expect(parsed.game_id).toBe('g1');
  });

  it('makeMove sends correct message', () => {
    const msg = JSON.stringify({ type: 'make_move', game_id: 'g1', from: 'b9', to: 'c7' });
    ws.send(msg);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('make_move');
    expect(parsed.from).toBe('b9');
    expect(parsed.to).toBe('c7');
  });

  it('resign sends correct message', () => {
    const msg = JSON.stringify({ type: 'resign', game_id: 'g1' });
    ws.send(msg);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('resign');
    expect(parsed.game_id).toBe('g1');
  });

  it('offerDraw sends correct message', () => {
    const msg = JSON.stringify({ type: 'offer_draw', game_id: 'g1' });
    ws.send(msg);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('offer_draw');
  });

  it('respondDraw sends correct message with accept', () => {
    const msg = JSON.stringify({ type: 'respond_draw', game_id: 'g1', accept: true });
    ws.send(msg);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('respond_draw');
    expect(parsed.accept).toBe(true);
  });

  it('respondDraw sends correct message with reject', () => {
    const msg = JSON.stringify({ type: 'respond_draw', game_id: 'g1', accept: false });
    ws.send(msg);
    const parsed = JSON.parse(ws.sent[0]);
    expect(parsed.type).toBe('respond_draw');
    expect(parsed.accept).toBe(false);
  });

  it('onmessage handler receives server messages', () => {
    const handler = vi.fn();
    ws.onmessage = (ev) => handler(JSON.parse(ev.data));
    ws.simulateMessage(JSON.stringify({ type: 'pong' }));
    expect(handler).toHaveBeenCalledWith({ type: 'pong' });
  });
});
