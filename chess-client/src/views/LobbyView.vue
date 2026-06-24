<template>
  <div>
    <nav class="nav-bar">
      <div>
        <router-link to="/lobby">大厅</router-link>
        <router-link to="/profile">个人资料</router-link>
      </div>
      <div class="user-info">
        {{ userStore.user?.username }} (评分: {{ userStore.user?.rating }})
        <button class="btn btn-secondary" style="margin-left: 12px;" @click="logout">登出</button>
      </div>
    </nav>
    <div class="container">
      <!-- 游戏列表 -->
      <div class="card" style="margin-bottom: 20px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <h2>对局大厅</h2>
          <button class="btn btn-primary" @click="showCreateDialog = true">创建对局</button>
        </div>

        <div style="margin-bottom: 12px;">
          <button class="btn btn-secondary" :style="{ fontWeight: filter === 'all' ? 'bold' : 'normal' }" @click="filter = 'all'">全部</button>
          <button class="btn btn-secondary" :style="{ fontWeight: filter === 'waiting' ? 'bold' : 'normal' }" @click="filter = 'waiting'">等待中</button>
          <button class="btn btn-secondary" :style="{ fontWeight: filter === 'playing' ? 'bold' : 'normal' }" @click="filter = 'playing'">进行中</button>
        </div>

        <div v-if="games.length === 0" style="color: #999; text-align: center; padding: 20px;">
          暂无对局
        </div>
        <div v-for="game in games" :key="game.id" style="padding: 12px; border-bottom: 1px solid #f0f0f0; display: flex; justify-content: space-between; align-items: center;">
          <div>
            <span style="font-weight: 500;">{{ game.red_player?.username || '?' }} vs {{ game.black_player?.username || '?' }}</span>
            <span style="margin-left: 8px; font-size: 12px; color: #999;">{{ game.status }}</span>
            <span v-if="game.time_control" style="margin-left: 8px; font-size: 12px; color: #666;">{{ Math.floor(game.time_control / 60) }}分</span>
            <span v-if="game.byoyomi" style="margin-left: 4px; font-size: 12px; color: #666;">+{{ game.byoyomi }}秒读秒</span>
          </div>
          <div>
            <button v-if="game.status === 'waiting'" class="btn btn-primary" @click="joinGame(game.id)">加入</button>
            <button v-if="game.status === 'playing'" class="btn btn-secondary" @click="watchGame(game.id)">观战</button>
            <button v-if="game.status === 'finished'" class="btn btn-secondary" @click="router.push(`/replay/${game.id}`)">回放</button>
          </div>
        </div>
      </div>

      <!-- 创建对局弹窗 -->
      <div v-if="showCreateDialog" style="position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.5); display: flex; align-items: center; justify-content: center;" @click.self="showCreateDialog = false">
        <div class="card" style="width: 400px;">
          <h3 style="margin-bottom: 16px;">创建对局</h3>
          <div class="form-group">
            <label>选择颜色</label>
            <select v-model="createColor" style="width: 100%; padding: 8px;">
              <option value="red">红方</option>
              <option value="black">黑方</option>
            </select>
          </div>
          <div class="form-group">
            <label>局时 (分钟)</label>
            <input v-model.number="createTimeControl" type="number" min="1" max="60" placeholder="留空不限时" />
          </div>
          <div class="form-group">
            <label>步时限 (秒)</label>
            <input v-model.number="createMoveTimeLimit" type="number" min="5" max="300" placeholder="留空不限" />
          </div>
          <div class="form-group">
            <label>读秒 (秒)</label>
            <input v-model.number="createByoyomi" type="number" min="3" max="60" placeholder="留空不读秒" />
          </div>
          <div style="display: flex; gap: 8px; margin-top: 16px;">
            <button class="btn btn-primary" style="flex: 1;" @click="handleCreate">创建</button>
            <button class="btn btn-secondary" style="flex: 1;" @click="showCreateDialog = false">取消</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '../stores/user';
import { useGameStore } from '../stores/game';
import { api } from '../api';
import { wsService } from '../api/websocket';
import type { Game, LobbyGameInfo } from '../types';

const router = useRouter();
const userStore = useUserStore();
const gameStore = useGameStore();

const games = ref<Game[]>([]);
const filter = ref('all');
const showCreateDialog = ref(false);
const createColor = ref('red');
const createTimeControl = ref<number | null>(null);
const createMoveTimeLimit = ref<number | null>(null);
const createByoyomi = ref<number | null>(null);

// WS message handler cleanup function
let unsubscribeWs: (() => void) | null = null;

async function loadGames() {
  try {
    const status = filter.value === 'all' ? undefined : filter.value;
    const res = await api.listGames(status);
    games.value = res.items;
  } catch {
    // ignore
  }
}

function handleLobbyUpdate(lobbyGames: LobbyGameInfo[]) {
  // Apply client-side status filter
  const status = filter.value;
  if (status === 'all') {
    games.value = lobbyGames as unknown as Game[];
  } else {
    games.value = lobbyGames.filter(g => g.status === status) as unknown as Game[];
  }
}

async function joinGame(gameId: string) {
  try {
    await gameStore.joinGame(gameId);
    router.push(`/game/${gameId}`);
  } catch (err: any) {
    alert(err.response?.data?.error || '加入失败');
  }
}

async function watchGame(gameId: string) {
  try {
    await gameStore.watchGame(gameId);
    router.push(`/game/${gameId}`);
  } catch (err: any) {
    alert(err.response?.data?.error || '观战失败');
  }
}

async function handleCreate() {
  try {
    await gameStore.createGame({
      player_color: createColor.value,
      time_control: createTimeControl.value ? createTimeControl.value * 60 : undefined,
      move_time_limit: createMoveTimeLimit.value || undefined,
      byoyomi: createByoyomi.value || undefined,
    });
    if (gameStore.currentGame) {
      router.push(`/game/${gameStore.currentGame.id}`);
    }
    showCreateDialog.value = false;
  } catch (err: any) {
    alert(err.response?.data?.error || '创建失败');
  }
}

function logout() {
  userStore.logout();
  router.push('/login');
}

onMounted(() => {
  // Initial REST load as fallback
  loadGames();

  // Subscribe to real-time lobby updates via WebSocket
  unsubscribeWs = wsService.onMessage((msg) => {
    if (msg.type === 'lobby_update') {
      handleLobbyUpdate(msg.games);
    }
  });
  wsService.subscribeLobby();
});
onUnmounted(() => {
  if (unsubscribeWs) {
    unsubscribeWs();
    unsubscribeWs = null;
  }
});
watch(filter, loadGames);
</script>