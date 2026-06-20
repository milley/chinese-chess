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
  const drawOfferedByMe = ref(false);
  const redTime = ref<number>(0);
  const blackTime = ref<number>(0);
  const redInByoyomi = ref<boolean>(false);
  const blackInByoyomi = ref<boolean>(false);
  const timerInterval = ref<ReturnType<typeof setInterval> | null>(null);
  const errorMessage = ref<string | null>(null);
  const errorTimeout = ref<ReturnType<typeof setTimeout> | null>(null);
  const isCheck = ref(false);
  const opponentDisconnected = ref(false);
  const lastMove = ref<{ from: string; to: string } | null>(null);

  const isMyTurn = computed(() => {
    if (!currentGame.value || !playerColor.value) return false;
    const fen = currentGame.value.fen;
    const sideToMove = fen.split(' ')[1] === 'w' ? 'red' : 'black';
    return sideToMove === playerColor.value;
  });

  /// Show an error message that auto-clears after 5 seconds.
  /// If another error is already showing, replace it (reset the timer).
  function showError(message: string) {
    errorMessage.value = message;
    // Clear any existing timeout
    if (errorTimeout.value) {
      clearTimeout(errorTimeout.value);
    }
    errorTimeout.value = setTimeout(() => {
      errorMessage.value = null;
      errorTimeout.value = null;
    }, 5000);
  }

  async function createGame(data: { player_color?: string; time_control?: number; move_time_limit?: number; byoyomi?: number }) {
    const res = await api.createGame(data);
    playerColor.value = res.color as 'red' | 'black';
    await loadGame(res.game_id);
    wsService.joinGame(res.game_id);
    startLocalTimer();
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

  /// Join WS room only (no REST joinGame call).
  /// Used by GameView on mount for direct URL / refresh — the player
  /// is already joined via DB, we just need the real-time WS channel.
  async function joinWsRoom(gameId: string) {
    if (!currentGame.value) return;

    // Determine player color from game data if not already set
    if (!playerColor.value) {
      const userId = JSON.parse(localStorage.getItem('user') || '{}').id;
      if (currentGame.value.red_player?.id === userId) {
        playerColor.value = 'red';
      } else if (currentGame.value.black_player?.id === userId) {
        playerColor.value = 'black';
      } else {
        playerColor.value = null;
        isSpectator.value = true;
      }
    }

    // Ensure WS is connected before joining the room.
    // On page refresh / direct URL access, the WS may not be connected yet.
    if (!wsService.isConnected) {
      const token = localStorage.getItem('token');
      if (token) {
        try {
          await wsService.connect(token);
        } catch {
          // WS connection failed — real-time updates won't work,
          // but the user can still see the board via REST data.
        }
      }
    }

    wsService.joinGame(gameId);
    startLocalTimer();
  }

  /// Enter spectator mode for a game (no REST joinGame, just load + WS).
  /// Used when a user clicks "观战" (watch) in the lobby.
  async function watchGame(gameId: string) {
    await loadGame(gameId);
    playerColor.value = null;
    isSpectator.value = true;
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
    if (currentGame.value.status !== 'playing') return;

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
    // Use WebSocket for real-time move delivery (server broadcasts to both players)
    wsService.makeMove(currentGame.value.id, from, to);
  }

  function handleWsMessage(message: WsServerMessage) {
    // Filter messages by game_id — ignore messages for other games
    const msgGameId = (message as any).game_id as string | undefined;
    if (msgGameId && currentGame.value && msgGameId !== currentGame.value.id) {
      return;
    }

    switch (message.type) {
      case 'joined_game':
        if (currentGame.value) {
          currentGame.value.fen = message.fen;
          // Don't override status to 'waiting' if game is already playing
          // (reconnect case: game is still in progress)
        } else {
          // First join — create minimal game object
          currentGame.value = {
            id: message.game_id,
            red_player: null,
            black_player: null,
            status: 'waiting',
            result: null,
            end_reason: null,
            fen: message.fen,
            initial_fen: null,
            time_control: null,
            move_time_limit: null,
            byoyomi: null,
            red_time: null,
            black_time: null,
            created_at: '',
          };
        }
        startLocalTimer();
        break;
      case 'opponent_joined':
        if (currentGame.value) {
          currentGame.value.status = 'playing';
          // Update opponent info from the message
          if (message.opponent) {
            const userId = JSON.parse(localStorage.getItem('user') || '{}').id;
            if (message.opponent.id !== userId) {
              if (playerColor.value === 'red') {
                currentGame.value.black_player = message.opponent;
              } else {
                currentGame.value.red_player = message.opponent;
              }
            }
          }
        }
        opponentDisconnected.value = false;
        break;
      case 'move_made':
        if (currentGame.value) {
          currentGame.value.fen = message.fen;
          moveHistory.value.push(`${message.from}-${message.to}`);
          // Sync time from server (authoritative)
          if (message.red_time !== undefined && message.red_time !== null) {
            redTime.value = message.red_time;
          }
          if (message.black_time !== undefined && message.black_time !== null) {
            blackTime.value = message.black_time;
          }
        }
        // Show check indicator
        isCheck.value = message.is_check;
        // Track last move for highlight
        lastMove.value = { from: message.from, to: message.to };
        // Auto-clear check indicator after 2 seconds
        if (message.is_check) {
          setTimeout(() => {
            isCheck.value = false;
          }, 2000);
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
        // Sync final time from server (authoritative) to prevent
        // displaying stale local timer values (e.g., showing 1s
        // remaining when the player actually timed out at 0s).
        if (message.red_time !== undefined && message.red_time !== null) {
          redTime.value = message.red_time;
        }
        if (message.black_time !== undefined && message.black_time !== null) {
          blackTime.value = message.black_time;
        }
        break;
      case 'time_update':
        // Server-authoritative time update — overwrite local values.
        // Only apply when the game is actually playing; ignore stale
        // updates that may arrive before the game has started.
        if (currentGame.value && currentGame.value.status === 'playing') {
          redTime.value = message.red_time;
          blackTime.value = message.black_time;
          redInByoyomi.value = message.red_in_byoyomi;
          blackInByoyomi.value = message.black_in_byoyomi;
        }
        break;
      case 'draw_offered':
        // Draw offer received — the offerer doesn't get this back,
        // only the opponent receives it.
        drawOffered.value = true;
        drawOfferedByMe.value = false;
        break;
      case 'draw_response':
        drawOffered.value = false;
        drawOfferedByMe.value = false;
        break;
      case 'illegal_move':
        showError(message.reason);
        break;
      case 'opponent_disconnected':
        opponentDisconnected.value = true;
        showError('对手已断线');
        break;
      case 'opponent_reconnected':
        // Handle opponent reconnect notification
        opponentDisconnected.value = false;
        break;
      case 'error':
        showError(message.message);
        break;
    }
  }

  function startLocalTimer() {
    stopLocalTimer();
    // Track the last server update timestamp to avoid flicker.
    // The local timer decrements, but the next TimeUpdate from the server
    // will overwrite with the authoritative value. To prevent the display
    // from jumping back up, we skip local decrement if the server update
    // just arrived (within the last 200ms).
    let lastServerUpdate = Date.now();

    // Override the time_update handler to track server update time
    const origHandler = handleWsMessage;

    timerInterval.value = setInterval(() => {
      if (!currentGame.value || currentGame.value.status !== 'playing') {
        stopLocalTimer();
        return;
      }
      // Local timer is for smooth display interpolation only.
      // The server broadcasts authoritative TimeUpdate every second,
      // which overwrites these values. This local decrement ensures
      // the display doesn't freeze between server updates.
      const active = currentGame.value.fen.split(' ')[1] === 'w' ? 'red' : 'black';
      const now = Date.now();
      // Only decrement locally if no server update came in the last 800ms
      // (server sends every 1s, so we get ~200ms overlap where local decrement
      // provides smooth interpolation, then server overwrites)
      if (now - lastServerUpdate > 200) {
        if (active === 'red') {
          redTime.value = Math.max(0, redTime.value - 1);
        } else {
          blackTime.value = Math.max(0, blackTime.value - 1);
        }
      }
    }, 1000);

    // Patch: listen for time_update to track last server update
    // We do this via a secondary listener on the WS service
    const unwatch = wsService.onMessage((msg: WsServerMessage) => {
      if (msg.type === 'time_update') {
        const msgGameId = (msg as any).game_id as string | undefined;
        if (!msgGameId || !currentGame.value || msgGameId === currentGame.value.id) {
          lastServerUpdate = Date.now();
        }
      }
    });
    // Store the unsubscribe function for cleanup
    (timerInterval as any)._unwatch = unwatch;
  }

  function stopLocalTimer() {
    if (timerInterval.value) {
      // Clean up the secondary WS listener
      const unwatch = (timerInterval as any)._unwatch;
      if (unwatch) unwatch();
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
      drawOffered.value = true;
      drawOfferedByMe.value = true;
    }
  }

  function respondDraw(accept: boolean) {
    if (currentGame.value) {
      wsService.respondDraw(currentGame.value.id, accept);
      drawOffered.value = false;
      drawOfferedByMe.value = false;
    }
  }

  /// Clean up state and timers when leaving a game view.
  /// Resets auxiliary state and stops the local timer.
  function cleanup() {
    stopLocalTimer();
    if (errorTimeout.value) {
      clearTimeout(errorTimeout.value);
      errorTimeout.value = null;
    }
    selectedSquare.value = null;
    validMoves.value = [];
    moveHistory.value = [];
    drawOffered.value = false;
    drawOfferedByMe.value = false;
    errorMessage.value = null;
    isCheck.value = false;
    opponentDisconnected.value = false;
    lastMove.value = null;
    currentGame.value = null;
    playerColor.value = null;
    isSpectator.value = false;
  }

  // Register WS message listener
  wsService.onMessage(handleWsMessage);

  // On WS reconnect, re-join the current game to sync state
  wsService.onReconnect(() => {
    if (currentGame.value && currentGame.value.status !== 'finished') {
      wsService.joinGame(currentGame.value.id);
      // Reload full game state from server on reconnect
      loadGame(currentGame.value.id);
    }
  });

  return {
    currentGame, playerColor, isSpectator, selectedSquare, validMoves,
    moveHistory, drawOffered, drawOfferedByMe, redTime, blackTime,
    redInByoyomi, blackInByoyomi, errorMessage, isMyTurn, isCheck,
    opponentDisconnected, lastMove,
    createGame, joinGame, joinWsRoom, watchGame, loadGame, selectSquare, makeMove,
    resign, offerDraw, respondDraw, cleanup,
  };
});
