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
      <div style="flex-shrink: 0; max-width: 100%;">
        <ChessBoard
          :fen="replayStore.currentFen"
          :player-color="null"
          :selected-square="null"
          :valid-moves="[]"
          :is-check="replayStore.currentMove?.is_check ?? false"
          :last-move="lastMoveFromReplay"
        />
      </div>

      <!-- 信息面板 -->
      <div class="card" style="flex: 1; min-width: 200px;">
        <h3 style="margin-bottom: 12px;">对局回放</h3>

        <!-- 对局信息 -->
        <div style="margin-bottom: 16px; font-size: 13px;">
          <div style="margin-bottom: 4px;">
            <strong>{{ replayStore.game?.red_player?.username || '?' }}</strong>
            <span style="color: #d4380d;"> (红方)</span>
            vs
            <strong>{{ replayStore.game?.black_player?.username || '?' }}</strong>
            <span style="color: #000;"> (黑方)</span>
          </div>
          <div v-if="replayStore.game?.time_control" style="color: #666;">
            局时: {{ Math.floor(replayStore.game.time_control / 60) }}分
            <span v-if="replayStore.game.byoyomi">+{{ replayStore.game.byoyomi }}秒读秒</span>
          </div>
          <div v-if="replayStore.game?.status === 'finished'" style="color: #d4380d; font-weight: bold;">
            结果: {{ formatResult(replayStore.game?.result) }}
            <span style="font-weight: normal; color: #666;">({{ formatEndReason(replayStore.game?.end_reason) }})</span>
          </div>
        </div>

        <!-- 当前走法详情 -->
        <div v-if="replayStore.currentMove" style="margin-bottom: 12px; font-size: 13px; padding: 8px; background: #f9f6f0; border-radius: 4px;">
          <div><strong>{{ replayStore.currentMove.color === 'red' ? '红方' : '黑方' }}</strong> 走棋 {{ replayStore.currentMove.notation || replayStore.currentMove.move }}</div>
          <div v-if="replayStore.currentMove.is_check" style="color: #d4380d; font-weight: bold;">⚠ 将军！</div>
          <div style="color: #666;">
            用时: {{ replayStore.currentMove.time_spent ?? '-' }}秒
            <span v-if="replayStore.currentMove.red_time != null">
              | 红方 {{ formatTime(replayStore.currentMove.red_time) }}
              | 黑方 {{ formatTime(replayStore.currentMove.black_time) }}
            </span>
          </div>
        </div>
        <div v-else style="margin-bottom: 12px; font-size: 13px; color: #999; padding: 8px; background: #f9f6f0; border-radius: 4px;">
          初始局面
        </div>

        <!-- 走法列表 -->
        <div style="margin-bottom: 16px;">
          <h4 style="margin-bottom: 8px;">走法历史</h4>
          <div class="move-list" style="max-height: 200px; overflow-y: auto; font-size: 13px;">
            <div
              v-for="(move, i) in replayStore.moves"
              :key="i"
              :class="{ 'move-highlight': i === replayStore.currentStep - 1 }"
              class="move-item"
              @click="replayStore.goToStep(i + 1)"
            >
              {{ Math.floor(i / 2) + 1 }}{{ i % 2 === 0 ? '.' : '...' }}
              <span :style="{ color: move.color === 'red' ? '#d4380d' : '#000' }">{{ move.color === 'red' ? '红' : '黑' }}</span>
              {{ move.notation || move.move }}
              <span v-if="move.is_check">+</span>
            </div>
          </div>
        </div>

        <!-- 回放控制 -->
        <ReplayControls
          :current-step="replayStore.currentStep"
          :total-steps="replayStore.totalSteps"
          :is-first-step="replayStore.isFirstStep"
          :is-last-step="replayStore.isLastStep"
          :is-auto-playing="replayStore.isAutoPlaying"
          :auto-play-speed="replayStore.autoPlaySpeed"
          @go-first="replayStore.goFirst()"
          @go-prev="replayStore.goPrev()"
          @go-next="replayStore.goNext()"
          @go-last="replayStore.goLast()"
          @toggle-auto-play="replayStore.isAutoPlaying ? replayStore.stopAutoPlay() : replayStore.startAutoPlay()"
          @update:current-step="replayStore.goToStep($event)"
          @update:auto-play-speed="replayStore.setAutoPlaySpeed($event)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue';
import { useRoute } from 'vue-router';
import { useUserStore } from '../stores/user';
import { useReplayStore } from '../stores/replay';
import ChessBoard from '../components/ChessBoard.vue';
import ReplayControls from '../components/ReplayControls.vue';

const route = useRoute();
const userStore = useUserStore();
const replayStore = useReplayStore();

const lastMoveFromReplay = computed<{ from: string; to: string } | null>(() => {
  const move = replayStore.currentMove;
  if (!move) return null;
  const parts = move.move.split('-');
  if (parts.length !== 2) return null;
  return { from: parts[0], to: parts[1] };
});

function formatTime(seconds: number | null | undefined): string {
  if (seconds == null) return '--:--';
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

function formatEndReason(reason: string | null | undefined): string {
  switch (reason) {
    case 'checkmate': return '将杀';
    case 'stalemate': return '困毙';
    case 'resign': return '认输';
    case 'timeout': return '超时';
    case 'draw_agreement': return '和棋协议';
    case 'disconnect': return '断线';
    default: return reason || '';
  }
}

function handleKeydown(e: KeyboardEvent) {
  switch (e.key) {
    case 'ArrowLeft':
      e.preventDefault();
      replayStore.goPrev();
      break;
    case 'ArrowRight':
      e.preventDefault();
      replayStore.goNext();
      break;
    case 'Home':
      e.preventDefault();
      replayStore.goFirst();
      break;
    case 'End':
      e.preventDefault();
      replayStore.goLast();
      break;
  }
}

onMounted(async () => {
  const gameId = route.params.id as string;
  if (gameId) {
    await replayStore.loadReplay(gameId);
  }
  document.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown);
  replayStore.cleanup();
});
</script>

<style scoped>
.move-list {
  border: 1px solid #eee;
  border-radius: 4px;
  padding: 4px;
}
.move-item {
  padding: 2px 6px;
  cursor: pointer;
  border-radius: 2px;
}
.move-item:hover {
  background: #f0f0f0;
}
.move-highlight {
  background: #fff3cd;
  font-weight: 500;
}
</style>
