import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useReplayStore } from '../../stores/replay';
import { createTestingPinia } from '@pinia/testing';

// Mock API
vi.mock('../../api', () => ({
  api: {
    getGame: vi.fn().mockResolvedValue({
      id: 'game-1',
      red_player: { id: 'u1', username: 'Red', display_name: null, rating: 1500, wins: 0, losses: 0, draws: 0 },
      black_player: { id: 'u2', username: 'Black', display_name: null, rating: 1500, wins: 0, losses: 0, draws: 0 },
      status: 'finished',
      result: 'red_win',
      end_reason: 'checkmate',
      fen: 'final-fen',
      initial_fen: 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1',
      time_control: 600,
      move_time_limit: null,
      byoyomi: null,
      red_time: 500,
      black_time: 480,
      created_at: '2024-01-01T00:00:00Z',
    }),
    getGameMoves: vi.fn().mockResolvedValue([
      { move: 'b9-c7', color: 'red', fen: 'fen-after-1', is_check: false, time_spent: 5, red_time: 595, black_time: 600, timestamp: '2024-01-01T00:00:05Z' },
      { move: 'b0-c2', color: 'black', fen: 'fen-after-2', is_check: false, time_spent: 8, red_time: 595, black_time: 592, timestamp: '2024-01-01T00:00:13Z' },
      { move: 'h9-g7', color: 'red', fen: 'fen-after-3', is_check: true, time_spent: 3, red_time: 592, black_time: 592, timestamp: '2024-01-01T00:00:16Z' },
    ]),
  },
}));

describe('replayStore', () => {
  let store: ReturnType<typeof useReplayStore>;

  beforeEach(() => {
    const pinia = createTestingPinia({ stubActions: false });
    store = useReplayStore(pinia);
  });

  describe('loadReplay', () => {
    it('loads game and moves, sets step to last', async () => {
      await store.loadReplay('game-1');
      expect(store.game).not.toBeNull();
      expect(store.moves).toHaveLength(3);
      expect(store.currentStep).toBe(3); // starts at the end
      expect(store.isLastStep).toBe(true);
    });
  });

  describe('step navigation', () => {
    beforeEach(async () => {
      await store.loadReplay('game-1');
    });

    it('goFirst goes to step 0', () => {
      store.goFirst();
      expect(store.currentStep).toBe(0);
      expect(store.isFirstStep).toBe(true);
    });

    it('goPrev decrements step', () => {
      store.goToStep(2);
      store.goPrev();
      expect(store.currentStep).toBe(1);
    });

    it('goNext increments step', () => {
      store.goFirst();
      store.goNext();
      expect(store.currentStep).toBe(1);
    });

    it('goLast goes to totalSteps', () => {
      store.goFirst();
      store.goLast();
      expect(store.currentStep).toBe(3);
      expect(store.isLastStep).toBe(true);
    });

    it('goToStep clamps to valid range', () => {
      store.goToStep(-1);
      expect(store.currentStep).toBe(0);
      store.goToStep(999);
      expect(store.currentStep).toBe(3);
    });
  });

  describe('currentFen', () => {
    beforeEach(async () => {
      await store.loadReplay('game-1');
    });

    it('step 0 returns initial_fen', () => {
      store.goFirst();
      expect(store.currentFen).toBe('rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1');
    });

    it('step N returns moves[N-1].fen', () => {
      store.goToStep(1);
      expect(store.currentFen).toBe('fen-after-1');
      store.goToStep(3);
      expect(store.currentFen).toBe('fen-after-3');
    });
  });

  describe('currentMove', () => {
    beforeEach(async () => {
      await store.loadReplay('game-1');
    });

    it('returns null at step 0', () => {
      store.goFirst();
      expect(store.currentMove).toBeNull();
    });

    it('returns move entry at step N', () => {
      store.goToStep(2);
      expect(store.currentMove).not.toBeNull();
      expect(store.currentMove!.move).toBe('b0-c2');
      expect(store.currentMove!.color).toBe('black');
    });
  });

  describe('auto-play', () => {
    beforeEach(async () => {
      await store.loadReplay('game-1');
      vi.useFakeTimers();
    });

    afterEach(() => {
      store.cleanup();
      vi.useRealTimers();
    });

    it('startAutoPlay advances steps', () => {
      store.goFirst();
      store.startAutoPlay();
      expect(store.isAutoPlaying).toBe(true);
      vi.advanceTimersByTime(1000);
      expect(store.currentStep).toBe(1);
      vi.advanceTimersByTime(1000);
      expect(store.currentStep).toBe(2);
    });

    it('stopAutoPlay stops advancing', () => {
      store.goFirst();
      store.startAutoPlay();
      vi.advanceTimersByTime(1000);
      store.stopAutoPlay();
      expect(store.isAutoPlaying).toBe(false);
      vi.advanceTimersByTime(2000);
      expect(store.currentStep).toBe(1); // didn't advance further
    });

    it('auto-play stops at last step', () => {
      store.goToStep(1);
      store.startAutoPlay();
      vi.advanceTimersByTime(3000); // enough to reach end
      expect(store.currentStep).toBe(3);
      expect(store.isAutoPlaying).toBe(false);
    });
  });

  describe('cleanup', () => {
    it('resets all state', async () => {
      await store.loadReplay('game-1');
      store.cleanup();
      expect(store.game).toBeNull();
      expect(store.moves).toEqual([]);
      expect(store.currentStep).toBe(0);
      expect(store.isAutoPlaying).toBe(false);
    });
  });
});
