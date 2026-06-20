<template>
  <div>
    <nav class="nav-bar">
      <div>
        <router-link to="/lobby">大厅</router-link>
        <router-link to="/profile">个人资料</router-link>
      </div>
      <div class="user-info">
        {{ userStore.user?.username }}
      </div>
    </nav>
    <div class="container" style="display: flex; gap: 20px;">
      <!-- 棋盘 -->
      <div>
        <ChessBoard
          :fen="gameStore.currentGame?.fen || ''"
          :player-color="gameStore.playerColor"
          :selected-square="gameStore.selectedSquare"
          :valid-moves="gameStore.validMoves"
          :is-check="gameStore.isCheck"
          :last-move="gameStore.lastMove"
          @square-click="gameStore.selectSquare"
        />
      </div>

      <!-- 信息面板 -->
      <div class="card" style="flex: 1; min-width: 200px;">
        <h3 style="margin-bottom: 12px;">对局信息</h3>

        <!-- 计时器 -->
        <div style="display: flex; justify-content: space-between; margin-bottom: 16px;">
          <div style="text-align: center;">
            <div style="font-size: 12px; color: #999;">红方</div>
            <div style="font-size: 24px; font-weight: bold;" :style="{ color: gameStore.redInByoyomi ? '#faad14' : '#d4380d' }">
              {{ formatTime(gameStore.redTime) }}
              <span v-if="gameStore.redInByoyomi" style="font-size: 12px; font-weight: normal;">读秒</span>
            </div>
          </div>
          <div style="text-align: center;">
            <div style="font-size: 12px; color: #999;">黑方</div>
            <div style="font-size: 24px; font-weight: bold;" :style="{ color: gameStore.blackInByoyomi ? '#faad14' : '#000' }">
              {{ formatTime(gameStore.blackTime) }}
              <span v-if="gameStore.blackInByoyomi" style="font-size: 12px; font-weight: normal;">读秒</span>
            </div>
          </div>
        </div>

        <!-- 状态 -->
        <div v-if="gameStore.currentGame?.status === 'waiting'" style="color: #faad14; margin-bottom: 12px;">等待对手加入...</div>
        <div v-if="gameStore.isCheck" style="color: #d4380d; margin-bottom: 12px; font-weight: bold; font-size: 16px;">
          ⚠ 将军！
        </div>
        <div v-if="gameStore.opponentDisconnected" style="color: #999; margin-bottom: 12px;">
          对手已断线，等待重连...
        </div>
        <div v-if="gameStore.currentGame?.status === 'finished'" style="color: #d4380d; margin-bottom: 12px; font-weight: bold;">
          游戏结束: {{ formatResult(gameStore.currentGame?.result) }}
          <router-link :to="`/replay/${gameStore.currentGame?.id}`" style="font-weight: normal; color: #1890ff; margin-left: 8px;">查看回放</router-link>
        </div>
        <div v-if="gameStore.errorMessage" class="error-message" style="margin-bottom: 12px;">{{ gameStore.errorMessage }}</div>

        <!-- 走法历史 -->
        <div style="margin-bottom: 16px;">
          <h4 style="margin-bottom: 8px;">走法历史</h4>
          <div style="max-height: 200px; overflow-y: auto; font-size: 13px;">
            <div v-for="(move, i) in gameStore.moveHistory" :key="i" style="padding: 2px 0;">
              {{ Math.floor(i / 2) + 1 }}. {{ move }}
            </div>
          </div>
        </div>

        <!-- 操作按钮 -->
        <div v-if="gameStore.currentGame?.status === 'playing' && !gameStore.isSpectator" style="display: flex; gap: 8px;">
          <button class="btn btn-danger" @click="gameStore.resign()">认输</button>
          <button class="btn btn-secondary" @click="gameStore.offerDraw()" :disabled="gameStore.drawOffered">
            {{ gameStore.drawOfferedByMe ? '已提议和棋' : '提议和棋' }}
          </button>
        </div>
        <!-- Draw offer received (only shown to the opponent who didn't offer) -->
        <div v-if="gameStore.drawOffered && !gameStore.drawOfferedByMe && gameStore.currentGame?.status === 'playing'" style="margin-top: 12px;">
          <p>对手提议和棋</p>
          <button class="btn btn-primary" @click="gameStore.respondDraw(true)">接受</button>
          <button class="btn btn-secondary" @click="gameStore.respondDraw(false)">拒绝</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { useUserStore } from '../stores/user';
import { useGameStore } from '../stores/game';
import ChessBoard from '../components/ChessBoard.vue';

const route = useRoute();
const userStore = useUserStore();
const gameStore = useGameStore();

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

function formatResult(result: string | null | undefined): string {
  switch (result) {
    case 'red_win': return '红方胜';
    case 'black_win': return '黑方胜';
    case 'draw': return '和棋';
    default: return result || '未知';
  }
}

onMounted(async () => {
  const gameId = route.params.id as string;
  if (gameId) {
    await gameStore.loadGame(gameId);
    // Join WS room for real-time updates (handles direct URL / refresh)
    gameStore.joinWsRoom(gameId);
  }
});

onUnmounted(() => {
  // Clean up timer and state when leaving the game view
  gameStore.cleanup();
});
</script>
