import { ref, computed } from 'vue';
import { defineStore } from 'pinia';
import { api } from '../api';
import { wsService } from '../api/websocket';
import type { Game, WsServerMessage } from '../types';

export const useGameStore = defineStore('game', () => {
  const currentGame = ref<Game | null>(null);
  const playerColor = ref<'red' | 'black' | null>(null);
  const isSpectator = ref(false);
  const selectedSquare = ref<string | null>(null);
  const validMoves = ref<string[]>([]);
  const moveHistory = ref<string[]>([]);
  const drawOffered = ref(false);
  const redTime = ref<number>(0);
  const blackTime = ref<number>(0);
  const timerInterval = ref<ReturnType<typeof setInterval> | null>(null);
  const errorMessage = ref<string | null>(null);

  const isMyTurn = computed(() => {
    if (!currentGame.value || !playerColor.value) return false;
    const fen = currentGame.value.fen;
    const sideToMove = fen.split(' ')[1] === 'w' ? 'red' : 'black';
    return sideToMove === playerColor.value;
  });

  async function createGame(data: { player_color?: string; time_control?: number; move_time_limit?: number; byoyomi?: number }) {
    const res = await api.createGame(data);
    playerColor.value = res.color as 'red' | 'black';
    await loadGame(res.game_id);
    wsService.joinGame(res.game_id);
  }

  async function joinGame(gameId: string) {
    const game = await api.joinGame(gameId);
    currentGame.value = game;
    // Determine player color
    const userId = JSON.parse(localStorage.getItem('user') || '{}').id;
    if (game.red_player?.id === userId) {
      playerColor.value = 'red';
    } else if (game.black_player?.id === userId) {
      playerColor.value = 'black';
    } else {
      playerColor.value = null;
      isSpectator.value = true;
    }
    wsService.joinGame(gameId);
    startLocalTimer();
  }

  async function loadGame(gameId: string) {
    const game = await api.getGame(gameId);
    currentGame.value = game;
    redTime.value = game.red_time ?? 0;
    blackTime.value = game.black_time ?? 0;
  }

  async function selectSquare(position: string) {
    if (!currentGame.value || isSpectator.value || !isMyTurn.value) return;

    // If clicking a valid move target, execute the move
    if (validMoves.value.includes(position) && selectedSquare.value) {
      await makeMove(selectedSquare.value, position);
      selectedSquare.value = null;
      validMoves.value = [];
      return;
    }

    // Select piece and get valid moves
    const fen = currentGame.value.fen;
    try {
      const moves = await api.getValidMoves(fen, position);
      if (moves.length > 0) {
        selectedSquare.value = position;
        validMoves.value = moves;
      } else {
        selectedSquare.value = null;
        validMoves.value = [];
      }
    } catch {
      selectedSquare.value = null;
      validMoves.value = [];
    }
  }

  async function makeMove(from: string, to: string) {
    if (!currentGame.value) return;
    try {
      const res = await api.makeMove(currentGame.value.id, from, to);
      if (currentGame.value) {
        currentGame.value.fen = res.fen;
      }
      errorMessage.value = null;
    } catch (err: any) {
      errorMessage.value = err.response?.data?.error || 'Move failed';
    }
  }

  function handleWsMessage(message: WsServerMessage) {
    switch (message.type) {
      case 'joined_game':
        currentGame.value = {
          ...currentGame.value!,
          fen: message.fen,
          status: 'waiting',
        };
        startLocalTimer();
        break;
      case 'opponent_joined':
        if (currentGame.value) {
          currentGame.value.status = 'playing';
        }
        break;
      case 'move_made':
        if (currentGame.value) {
          currentGame.value.fen = message.fen;
          moveHistory.value.push(`${message.from}-${message.to}`);
        }
        selectedSquare.value = null;
        validMoves.value = [];
        break;
      case 'game_over':
        stopLocalTimer();
        if (currentGame.value) {
          currentGame.value.status = 'finished';
          currentGame.value.result = message.result as any;
        }
        break;
      case 'time_update':
        redTime.value = message.red_time;
        blackTime.value = message.black_time;
        break;
      case 'draw_offered':
        drawOffered.value = true;
        break;
      case 'draw_response':
        drawOffered.value = false;
        break;
      case 'illegal_move':
        errorMessage.value = message.reason;
        break;
      case 'opponent_disconnected':
        errorMessage.value = '对手已断线';
        break;
      case 'error':
        errorMessage.value = message.message;
        break;
    }
  }

  function startLocalTimer() {
    stopLocalTimer();
    timerInterval.value = setInterval(() => {
      if (!currentGame.value || currentGame.value.status !== 'playing') {
        stopLocalTimer();
        return;
      }
      const active = currentGame.value.fen.split(' ')[1] === 'w' ? 'red' : 'black';
      if (active === 'red') {
        redTime.value = Math.max(0, redTime.value - 1);
      } else {
        blackTime.value = Math.max(0, blackTime.value - 1);
      }
    }, 1000);
  }

  function stopLocalTimer() {
    if (timerInterval.value) {
      clearInterval(timerInterval.value);
      timerInterval.value = null;
    }
  }

  function resign() {
    if (currentGame.value) {
      wsService.resign(currentGame.value.id);
    }
  }

  function offerDraw() {
    if (currentGame.value) {
      wsService.offerDraw(currentGame.value.id);
    }
  }

  function respondDraw(accept: boolean) {
    if (currentGame.value) {
      wsService.respondDraw(currentGame.value.id, accept);
      drawOffered.value = false;
    }
  }

  // Register WS message listener
  wsService.onMessage(handleWsMessage);

  return {
    currentGame, playerColor, isSpectator, selectedSquare, validMoves,
    moveHistory, drawOffered, redTime, blackTime, errorMessage, isMyTurn,
    createGame, joinGame, loadGame, selectSquare, makeMove,
    resign, offerDraw, respondDraw,
  };
});