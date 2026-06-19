import { ref, computed } from 'vue';
import { defineStore } from 'pinia';
import { api } from '../api';
import type { Game, MoveEntry } from '../types';

const INITIAL_FEN = 'rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1';

export const useReplayStore = defineStore('replay', () => {
  const game = ref<Game | null>(null);
  const moves = ref<MoveEntry[]>([]);
  const currentStep = ref(0); // 0 = initial position, N = after move N-1
  const isAutoPlaying = ref(false);
  const autoPlaySpeed = ref(1000); // ms between steps
  const autoPlayTimer = ref<ReturnType<typeof setInterval> | null>(null);

  const totalSteps = computed(() => moves.value.length);

  const currentFen = computed(() => {
    if (currentStep.value === 0) {
      return game.value?.initial_fen || INITIAL_FEN;
    }
    return moves.value[currentStep.value - 1]?.fen || INITIAL_FEN;
  });

  const currentMove = computed<MoveEntry | null>(() => {
    if (currentStep.value === 0) return null;
    return moves.value[currentStep.value - 1] ?? null;
  });

  const isFirstStep = computed(() => currentStep.value === 0);
  const isLastStep = computed(() => currentStep.value === totalSteps.value);

  async function loadReplay(gameId: string) {
    const gameData = await api.getGame(gameId);
    game.value = gameData;
    moves.value = await api.getGameMoves(gameId);
    currentStep.value = moves.value.length; // Start at the final position
  }

  function goToStep(step: number) {
    currentStep.value = Math.max(0, Math.min(step, totalSteps.value));
  }

  function goFirst() { goToStep(0); }
  function goPrev() { goToStep(currentStep.value - 1); }
  function goNext() {
    goToStep(currentStep.value + 1);
    if (isLastStep.value) stopAutoPlay();
  }
  function goLast() { goToStep(totalSteps.value); }

  function startAutoPlay() {
    if (isAutoPlaying.value) return;
    // If already at the last step, restart from beginning
    if (isLastStep.value) goToStep(0);
    isAutoPlaying.value = true;
    autoPlayTimer.value = setInterval(() => {
      if (isLastStep.value) {
        stopAutoPlay();
        return;
      }
      goNext();
    }, autoPlaySpeed.value);
  }

  function stopAutoPlay() {
    isAutoPlaying.value = false;
    if (autoPlayTimer.value) {
      clearInterval(autoPlayTimer.value);
      autoPlayTimer.value = null;
    }
  }

  function setAutoPlaySpeed(ms: number) {
    autoPlaySpeed.value = ms;
    // If auto-playing, restart with new speed
    if (isAutoPlaying.value) {
      stopAutoPlay();
      startAutoPlay();
    }
  }

  function cleanup() {
    stopAutoPlay();
    game.value = null;
    moves.value = [];
    currentStep.value = 0;
  }

  return {
    game, moves, currentStep, isAutoPlaying, autoPlaySpeed,
    totalSteps, currentFen, currentMove, isFirstStep, isLastStep,
    loadReplay, goToStep, goFirst, goPrev, goNext, goLast,
    startAutoPlay, stopAutoPlay, setAutoPlaySpeed, cleanup,
  };
});
