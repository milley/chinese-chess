import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { useGameStore } from '../../stores/game';
import { createTestingPinia } from '@pinia/testing';
import type { Game } from '../../types';

// Mock API module
vi.mock('../../api', () => ({
  api: {
    getValidMoves: vi.fn().mockResolvedValue(['a7', 'c7', 'b8']),
    getGame: vi.fn().mockResolvedValue(null),
  },
}));

// Mock WS module
vi.mock('../../api/websocket', () => ({
  wsService: {
    isConnected: true,
    joinGame: vi.fn(),
    makeMove: vi.fn(),
    offerDraw: vi.fn(),
    respondDraw: vi.fn(),
    resign: vi.fn(),
    leaveGame: vi.fn(),
    onMessage: vi.fn().mockReturnValue(() => {}),
    onConnect: vi.fn().mockReturnValue(() => {}),
    onDisconnect: vi.fn().mockReturnValue(() => {}),
    onReconnect: vi.fn().mockReturnValue(() => {}),
    connect: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn(),
    send: vi.fn(),
  },
}));

const INITIAL_FEN = 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1';

function createMockGame(overrides: Partial<Game> = {}): Game {
  return {
    id: 'game-123',
    red_player: { id: 'user-red', username: 'red', display_name: null, rating: 1500, wins: 0, losses: 0, draws: 0 },
    black_player: { id: 'user-black', username: 'black', display_name: null, rating: 1500, wins: 0, losses: 0, draws: 0 },
    status: 'playing',
    result: null,
    end_reason: null,
    fen: INITIAL_FEN,
    initial_fen: INITIAL_FEN,
    time_control: 600,
    move_time_limit: null,
    byoyomi: null,
    red_time: 600,
    black_time: 600,
    created_at: '2024-01-01T00:00:00Z',
    ...overrides,
  };
}

describe('gameStore', () => {
  let store: ReturnType<typeof useGameStore>;

  beforeEach(() => {
    const pinia = createTestingPinia({ stubActions: false });
    store = useGameStore(pinia);
  });

  describe('isMyTurn', () => {
    it('returns true when player is red and red to move', () => {
      store.currentGame = createMockGame({ fen: INITIAL_FEN }); // "w" = red's turn
      store.playerColor = 'red';
      expect(store.isMyTurn).toBe(true);
    });

    it('returns false when player is red and black to move', () => {
      const blackFen = INITIAL_FEN.replace(' w ', ' b ');
      store.currentGame = createMockGame({ fen: blackFen });
      store.playerColor = 'red';
      expect(store.isMyTurn).toBe(false);
    });

    it('returns false when no game', () => {
      store.currentGame = null;
      store.playerColor = 'red';
      expect(store.isMyTurn).toBe(false);
    });

    it('returns false for spectator', () => {
      store.currentGame = createMockGame();
      store.playerColor = null;
      store.isSpectator = true;
      expect(store.isMyTurn).toBe(false);
    });
  });

  describe('cleanup', () => {
    it('resets all state', () => {
      store.currentGame = createMockGame();
      store.playerColor = 'red';
      store.selectedSquare = 'a9';
      store.validMoves = ['a8', 'b8'];
      store.moveHistory = ['b9-c7'];
      store.drawOffered = true;
      store.drawOfferedByMe = true;
      store.errorMessage = 'error';
      store.isCheck = true;
      store.opponentDisconnected = true;

      store.cleanup();

      expect(store.currentGame).toBeNull();
      expect(store.playerColor).toBeNull();
      expect(store.selectedSquare).toBeNull();
      expect(store.validMoves).toEqual([]);
      expect(store.moveHistory).toEqual([]);
      expect(store.drawOffered).toBe(false);
      expect(store.drawOfferedByMe).toBe(false);
      expect(store.errorMessage).toBeNull();
      expect(store.isCheck).toBe(false);
      expect(store.opponentDisconnected).toBe(false);
    });
  });

  describe('offerDraw / respondDraw', () => {
    it('offerDraw sets both flags', () => {
      store.currentGame = createMockGame();
      store.offerDraw();
      expect(store.drawOffered).toBe(true);
      expect(store.drawOfferedByMe).toBe(true);
    });

    it('respondDraw clears both flags', () => {
      store.currentGame = createMockGame();
      store.drawOffered = true;
      store.drawOfferedByMe = true;
      store.respondDraw(true);
      expect(store.drawOffered).toBe(false);
      expect(store.drawOfferedByMe).toBe(false);
    });
  });

  describe('selectSquare', () => {
    it('does not select piece when game status is waiting', async () => {
      store.currentGame = createMockGame({ status: 'waiting' });
      store.playerColor = 'red';

      await store.selectSquare('b7');

      expect(store.selectedSquare).toBeNull();
      expect(store.validMoves).toEqual([]);
    });

    it('does not select piece when game status is finished', async () => {
      store.currentGame = createMockGame({ status: 'finished' });
      store.playerColor = 'red';

      await store.selectSquare('b7');

      expect(store.selectedSquare).toBeNull();
      expect(store.validMoves).toEqual([]);
    });

    it('selects piece and fetches valid moves when status is playing', async () => {
      store.currentGame = createMockGame();
      store.playerColor = 'red';

      await store.selectSquare('b7');

      expect(store.selectedSquare).toBe('b7');
      expect(store.validMoves).toEqual(['a7', 'c7', 'b8']);
    });

    it('does not select piece when not my turn', async () => {
      const blackFen = INITIAL_FEN.replace(' w ', ' b ');
      store.currentGame = createMockGame({ fen: blackFen });
      store.playerColor = 'red';

      await store.selectSquare('b7');

      expect(store.selectedSquare).toBeNull();
    });
  });
});
